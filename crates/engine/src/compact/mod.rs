pub mod microcompact;
pub mod reactive;

use crate::model_context::context_window_for_model;
use crate::session::{ContentBlock, ConversationMessage, MessageRole, Session};

const COMPACT_CONTINUATION_PREAMBLE: &str =
    "This session is being continued from a previous conversation that ran out of context. The summary below covers the earlier portion of the conversation.\n\n";
const COMPACT_RECENT_MESSAGES_NOTE: &str = "Recent messages are preserved verbatim.";
const COMPACT_DIRECT_RESUME_INSTRUCTION: &str = "Continue the conversation from where it left off without asking the user any further questions. Resume directly — do not acknowledge the summary, do not recap what was happening, and do not preface with continuation text.";

/// System prompt for model-assisted compaction summaries (caller may override via [`ModelCompactionConfig::summary_prompt`]).
pub const COMPACT_SUMMARY_SYSTEM_PROMPT: &str = "You are a conversation summarizer. Given the previous messages from a coding assistant conversation, create a concise but comprehensive summary that preserves:
1. Key decisions made and their rationale
2. Files that were modified, created, or read
3. Tools that were used and their outcomes
4. Current task state and any pending work
5. Important context that would be needed to continue the conversation

Output ONLY the summary text, no preamble.";

/// Configuration for model-assisted compaction.
#[derive(Debug, Clone)]
pub struct ModelCompactionConfig {
    /// Whether to use model-assisted compaction (requires [`crate::conversation::ApiClient`]).
    pub enabled: bool,
    /// Maximum tokens to use for the compaction summary call (output budget).
    pub max_summary_tokens: usize,
    /// System prompt for the summarization model.
    pub summary_prompt: String,
}

impl Default for ModelCompactionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_summary_tokens: 2048,
            summary_prompt: COMPACT_SUMMARY_SYSTEM_PROMPT.to_string(),
        }
    }
}

/// Configuration for heuristic compaction of older messages.
///
/// [`should_compact`] uses fixed `max_estimated_tokens` plus `preserve_recent_messages`.
/// For model-specific context limits, use [`should_compact_for_model`] with the active model id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactionConfig {
    pub preserve_recent_messages: usize,
    pub max_estimated_tokens: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            preserve_recent_messages: 4,
            max_estimated_tokens: 10_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionResult {
    pub summary: String,
    pub formatted_summary: String,
    pub compacted_session: Session,
    pub removed_message_count: usize,
}

/// Result of applying a model-generated compaction summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCompactionResult {
    pub summary: String,
    pub compacted_session: Session,
    pub removed_message_count: usize,
}

/// Build the messages that would be sent to the model for compaction.
/// The actual API call is done by the caller ([`crate::conversation::ConversationRuntime`]).
///
/// Returns [`None`] when there is nothing to compact ([`should_compact`] is false or there are no
/// removable messages).
#[must_use]
pub fn build_model_compact_request(
    session: &Session,
    config: &CompactionConfig,
) -> Option<(Vec<ConversationMessage>, usize)> {
    if !should_compact(session, *config) {
        return None;
    }

    let existing_summary = session
        .messages
        .first()
        .and_then(extract_existing_compacted_summary);
    let compacted_prefix_len = usize::from(existing_summary.is_some());
    let keep_from = session
        .messages
        .len()
        .saturating_sub(config.preserve_recent_messages);
    if keep_from <= compacted_prefix_len {
        return None;
    }

    let removed = &session.messages[compacted_prefix_len..keep_from];
    let user_text = format!(
        "Summarize the following conversation segment for session compaction.\n\n---\n{}",
        removed_messages_as_text(removed)
    );
    Some((vec![ConversationMessage::user_text(user_text)], keep_from))
}

/// Apply a model-generated summary to the session (same structure as [`compact_session`]).
#[must_use]
pub fn apply_model_compact_summary(
    session: &Session,
    summary: &str,
    preserve_from: usize,
) -> ModelCompactionResult {
    let existing_summary = session
        .messages
        .first()
        .and_then(extract_existing_compacted_summary);
    let compacted_prefix_len = usize::from(existing_summary.is_some());
    let preserved = session.messages[preserve_from.min(session.messages.len())..].to_vec();
    let removed_message_count = preserve_from.saturating_sub(compacted_prefix_len);

    let merged = merge_compact_summaries(existing_summary.as_deref(), summary.trim());
    let continuation = get_compact_continuation_message(&merged, true, !preserved.is_empty());

    let mut compacted_messages = vec![ConversationMessage {
        role: MessageRole::System,
        blocks: vec![ContentBlock::Text { text: continuation }],
        usage: None,
    }];
    compacted_messages.extend(preserved);

    ModelCompactionResult {
        summary: merged,
        compacted_session: Session {
            version: session.version,
            messages: compacted_messages,
            cwd: session.cwd.clone(),
            model_id: session.model_id.clone(),
            created_at: session.created_at.clone(),
        },
        removed_message_count,
    }
}

#[must_use]
pub fn estimate_session_tokens(session: &Session) -> usize {
    session.messages.iter().map(estimate_message_tokens).sum()
}

#[must_use]
pub fn should_compact(session: &Session, config: CompactionConfig) -> bool {
    let start = compacted_summary_prefix_len(session);
    let compactable = &session.messages[start..];

    compactable.len() > config.preserve_recent_messages
        && compactable
            .iter()
            .map(estimate_message_tokens)
            .sum::<usize>()
            >= config.max_estimated_tokens
}

/// Returns true when estimated session size meets or exceeds the model's effective input budget.
#[must_use]
pub fn should_compact_for_model(session: &Session, model: &str) -> bool {
    let window = context_window_for_model(model);
    let tokens = estimate_session_tokens(session);
    tokens >= window.effective_input_budget()
}

#[must_use]
pub fn format_compact_summary(summary: &str) -> String {
    let without_analysis = strip_tag_block(summary, "analysis");
    let formatted = if let Some(content) = extract_tag_block(&without_analysis, "summary") {
        without_analysis.replace(
            &format!("<summary>{content}</summary>"),
            &format!("Summary:\n{}", content.trim()),
        )
    } else {
        without_analysis
    };

    collapse_blank_lines(&formatted).trim().to_string()
}

#[must_use]
pub fn get_compact_continuation_message(
    summary: &str,
    suppress_follow_up_questions: bool,
    recent_messages_preserved: bool,
) -> String {
    let mut base = format!(
        "{COMPACT_CONTINUATION_PREAMBLE}{}",
        format_compact_summary(summary)
    );

    if recent_messages_preserved {
        base.push_str("\n\n");
        base.push_str(COMPACT_RECENT_MESSAGES_NOTE);
    }

    if suppress_follow_up_questions {
        base.push('\n');
        base.push_str(COMPACT_DIRECT_RESUME_INSTRUCTION);
    }

    base
}

#[must_use]
pub fn compact_session(session: &Session, config: CompactionConfig) -> CompactionResult {
    if !should_compact(session, config) {
        return CompactionResult {
            summary: String::new(),
            formatted_summary: String::new(),
            compacted_session: session.clone(),
            removed_message_count: 0,
        };
    }

    let existing_summary = session
        .messages
        .first()
        .and_then(extract_existing_compacted_summary);
    let compacted_prefix_len = usize::from(existing_summary.is_some());
    let keep_from = session
        .messages
        .len()
        .saturating_sub(config.preserve_recent_messages);
    let removed = &session.messages[compacted_prefix_len..keep_from];
    let preserved = session.messages[keep_from..].to_vec();
    let summary =
        merge_compact_summaries(existing_summary.as_deref(), &summarize_messages(removed));
    let formatted_summary = format_compact_summary(&summary);
    let continuation = get_compact_continuation_message(&summary, true, !preserved.is_empty());

    let mut compacted_messages = vec![ConversationMessage {
        role: MessageRole::System,
        blocks: vec![ContentBlock::Text { text: continuation }],
        usage: None,
    }];
    compacted_messages.extend(preserved);

    CompactionResult {
        summary,
        formatted_summary,
        compacted_session: Session {
            version: session.version,
            messages: compacted_messages,
            cwd: session.cwd.clone(),
            model_id: session.model_id.clone(),
            created_at: session.created_at.clone(),
        },
        removed_message_count: removed.len(),
    }
}

fn removed_messages_as_text(messages: &[ConversationMessage]) -> String {
    messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            let role = match message.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
            };
            let body = message
                .blocks
                .iter()
                .map(summarize_block)
                .collect::<Vec<_>>()
                .join("\n");
            format!("## Message {index} ({role})\n{body}")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn compacted_summary_prefix_len(session: &Session) -> usize {
    usize::from(
        session
            .messages
            .first()
            .and_then(extract_existing_compacted_summary)
            .is_some(),
    )
}

#[derive(Debug, Clone, Copy)]
struct MessageStats {
    total: usize,
    user: usize,
    assistant: usize,
    tool: usize,
}

fn count_message_stats(messages: &[ConversationMessage]) -> MessageStats {
    MessageStats {
        total: messages.len(),
        user: messages
            .iter()
            .filter(|message| message.role == MessageRole::User)
            .count(),
        assistant: messages
            .iter()
            .filter(|message| message.role == MessageRole::Assistant)
            .count(),
        tool: messages
            .iter()
            .filter(|message| message.role == MessageRole::Tool)
            .count(),
    }
}

fn format_tool_summary(messages: &[ConversationMessage]) -> String {
    let mut tool_names = messages
        .iter()
        .flat_map(|message| message.blocks.iter())
        .filter_map(|block| match block {
            ContentBlock::ToolUse { name, .. } => Some(name.as_str()),
            ContentBlock::ToolResult { tool_name, .. } => Some(tool_name.as_str()),
            ContentBlock::Text { .. } | ContentBlock::Image { .. } => None,
        })
        .collect::<Vec<_>>();
    tool_names.sort_unstable();
    tool_names.dedup();
    if tool_names.is_empty() {
        String::new()
    } else {
        format!("- Tools mentioned: {}.", tool_names.join(", "))
    }
}

fn summarize_messages(messages: &[ConversationMessage]) -> String {
    let stats = count_message_stats(messages);

    let mut lines = vec![
        "<summary>".to_string(),
        "Conversation summary:".to_string(),
        format!(
            "- Scope: {} earlier messages compacted (user={}, assistant={}, tool={}).",
            stats.total, stats.user, stats.assistant, stats.tool
        ),
    ];

    let tool_line = format_tool_summary(messages);
    if !tool_line.is_empty() {
        lines.push(tool_line);
    }

    let recent_user_requests = collect_recent_role_summaries(messages, MessageRole::User, 3);
    if !recent_user_requests.is_empty() {
        lines.push("- Recent user requests:".to_string());
        lines.extend(
            recent_user_requests
                .into_iter()
                .map(|request| format!("  - {request}")),
        );
    }

    let pending_work = infer_pending_work(messages);
    if !pending_work.is_empty() {
        lines.push("- Pending work:".to_string());
        lines.extend(pending_work.into_iter().map(|item| format!("  - {item}")));
    }

    let key_files = collect_key_files(messages);
    if !key_files.is_empty() {
        lines.push(format!("- Key files referenced: {}.", key_files.join(", ")));
    }

    if let Some(current_work) = infer_current_work(messages) {
        lines.push(format!("- Current work: {current_work}"));
    }

    lines.push("- Key timeline:".to_string());
    for message in messages {
        let role = match message.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };
        let content = message
            .blocks
            .iter()
            .map(summarize_block)
            .collect::<Vec<_>>()
            .join(" | ");
        lines.push(format!("  - {role}: {content}"));
    }
    lines.push("</summary>".to_string());
    lines.join("\n")
}

fn merge_compact_summaries(existing_summary: Option<&str>, new_summary: &str) -> String {
    let Some(existing_summary) = existing_summary else {
        return new_summary.to_string();
    };

    let previous_highlights = extract_summary_highlights(existing_summary);
    let new_formatted_summary = format_compact_summary(new_summary);
    let new_highlights = extract_summary_highlights(&new_formatted_summary);
    let new_timeline = extract_summary_timeline(&new_formatted_summary);

    let mut lines = vec!["<summary>".to_string(), "Conversation summary:".to_string()];

    if !previous_highlights.is_empty() {
        lines.push("- Previously compacted context:".to_string());
        lines.extend(
            previous_highlights
                .into_iter()
                .map(|line| format!("  {line}")),
        );
    }

    if !new_highlights.is_empty() {
        lines.push("- Newly compacted context:".to_string());
        lines.extend(new_highlights.into_iter().map(|line| format!("  {line}")));
    }

    if !new_timeline.is_empty() {
        lines.push("- Key timeline:".to_string());
        lines.extend(new_timeline.into_iter().map(|line| format!("  {line}")));
    }

    lines.push("</summary>".to_string());
    lines.join("\n")
}

fn summarize_block(block: &ContentBlock) -> String {
    let raw = match block {
        ContentBlock::Text { text } => text.clone(),
        ContentBlock::Image { media_type, .. } => format!("[image: {media_type}]"),
        ContentBlock::ToolUse { name, input, .. } => format!("tool_use {name}({input})"),
        ContentBlock::ToolResult {
            tool_name,
            output,
            is_error,
            ..
        } => format!(
            "tool_result {tool_name}: {}{output}",
            if *is_error { "error " } else { "" }
        ),
    };
    truncate_summary(&raw, 160)
}

fn collect_recent_role_summaries(
    messages: &[ConversationMessage],
    role: MessageRole,
    limit: usize,
) -> Vec<String> {
    messages
        .iter()
        .filter(|message| message.role == role)
        .rev()
        .filter_map(|message| first_text_block(message))
        .take(limit)
        .map(|text| truncate_summary(text, 160))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn infer_pending_work(messages: &[ConversationMessage]) -> Vec<String> {
    messages
        .iter()
        .rev()
        .filter_map(first_text_block)
        .filter(|text| {
            let lowered = text.to_ascii_lowercase();
            lowered.contains("todo")
                || lowered.contains("next")
                || lowered.contains("pending")
                || lowered.contains("follow up")
                || lowered.contains("remaining")
        })
        .take(3)
        .map(|text| truncate_summary(text, 160))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn collect_key_files(messages: &[ConversationMessage]) -> Vec<String> {
    let mut files = messages
        .iter()
        .flat_map(|message| message.blocks.iter())
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::ToolUse { input, .. } => Some(input.as_str()),
            ContentBlock::ToolResult { output, .. } => Some(output.as_str()),
            ContentBlock::Image { .. } => None,
        })
        .flat_map(extract_file_candidates)
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
    files.into_iter().take(8).collect()
}

fn infer_current_work(messages: &[ConversationMessage]) -> Option<String> {
    messages
        .iter()
        .rev()
        .filter_map(first_text_block)
        .find(|text| !text.trim().is_empty())
        .map(|text| truncate_summary(text, 200))
}

fn first_text_block(message: &ConversationMessage) -> Option<&str> {
    message.blocks.iter().find_map(|block| match block {
        ContentBlock::Text { text } if !text.trim().is_empty() => Some(text.as_str()),
        ContentBlock::ToolUse { .. }
        | ContentBlock::ToolResult { .. }
        | ContentBlock::Image { .. }
        | ContentBlock::Text { .. } => None,
    })
}

fn has_interesting_extension(candidate: &str) -> bool {
    std::path::Path::new(candidate)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["rs", "ts", "tsx", "js", "json", "md"]
                .iter()
                .any(|expected| extension.eq_ignore_ascii_case(expected))
        })
}

fn extract_file_candidates(content: &str) -> Vec<String> {
    content
        .split_whitespace()
        .filter_map(|token| {
            let candidate = token.trim_matches(|char: char| {
                matches!(char, ',' | '.' | ':' | ';' | ')' | '(' | '"' | '\'' | '`')
            });
            if (candidate.contains('/') || candidate.contains('\\'))
                && has_interesting_extension(candidate)
            {
                Some(candidate.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn truncate_summary(content: &str, max_chars: usize) -> String {
    if content.chars().count() <= max_chars {
        return content.to_string();
    }
    let mut truncated = content.chars().take(max_chars).collect::<String>();
    truncated.push('…');
    truncated
}

/// Heuristic token estimate for a single string (UTF-8 aware).
#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
    // Byte-based estimation is more accurate than char-based for multilingual content
    // ~4 bytes per token for English, ~2-3 for CJK
    let byte_len = text.len();
    let char_len = text.chars().count();
    // If mostly ASCII, use byte_len / 4; if CJK-heavy, use char_len / 1.5
    let ascii_ratio = byte_len as f64 / char_len.max(1) as f64;
    if ascii_ratio > 1.5 {
        // Mostly CJK/multi-byte
        (char_len as f64 / 1.5).ceil() as usize + 1
    } else {
        byte_len / 4 + 1
    }
}

fn estimate_message_tokens(message: &ConversationMessage) -> usize {
    message
        .blocks
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => estimate_tokens(text),
            ContentBlock::Image { data, .. } => estimate_tokens(data),
            ContentBlock::ToolUse { name, input, .. } => {
                estimate_tokens(name.as_str()).saturating_add(estimate_tokens(input.as_str()))
            }
            ContentBlock::ToolResult {
                tool_name, output, ..
            } => {
                estimate_tokens(tool_name.as_str()).saturating_add(estimate_tokens(output.as_str()))
            }
        })
        .sum()
}

fn extract_tag_block(content: &str, tag: &str) -> Option<String> {
    let start = format!("<{tag}>");
    let end = format!("</{tag}>");
    let start_index = content.find(&start)? + start.len();
    let end_index = content[start_index..].find(&end)? + start_index;
    Some(content[start_index..end_index].to_string())
}

fn strip_tag_block(content: &str, tag: &str) -> String {
    let start = format!("<{tag}>");
    let end = format!("</{tag}>");
    if let (Some(start_index), Some(end_index_rel)) = (content.find(&start), content.find(&end)) {
        let end_index = end_index_rel + end.len();
        let mut stripped = String::new();
        stripped.push_str(&content[..start_index]);
        stripped.push_str(&content[end_index..]);
        stripped
    } else {
        content.to_string()
    }
}

fn collapse_blank_lines(content: &str) -> String {
    let mut result = String::new();
    let mut last_blank = false;
    for line in content.lines() {
        let is_blank = line.trim().is_empty();
        if is_blank && last_blank {
            continue;
        }
        result.push_str(line);
        result.push('\n');
        last_blank = is_blank;
    }
    result
}

fn extract_existing_compacted_summary(message: &ConversationMessage) -> Option<String> {
    if message.role != MessageRole::System {
        return None;
    }

    let text = first_text_block(message)?;
    let summary = text.strip_prefix(COMPACT_CONTINUATION_PREAMBLE)?;
    let summary = summary
        .split_once(&format!("\n\n{COMPACT_RECENT_MESSAGES_NOTE}"))
        .map_or(summary, |(value, _)| value);
    let summary = summary
        .split_once(&format!("\n{COMPACT_DIRECT_RESUME_INSTRUCTION}"))
        .map_or(summary, |(value, _)| value);
    Some(summary.trim().to_string())
}

fn extract_summary_highlights(summary: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut in_timeline = false;

    for line in format_compact_summary(summary).lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() || trimmed == "Summary:" || trimmed == "Conversation summary:" {
            continue;
        }
        if trimmed == "- Key timeline:" {
            in_timeline = true;
            continue;
        }
        if in_timeline {
            continue;
        }
        lines.push(trimmed.to_string());
    }

    lines
}

fn extract_summary_timeline(summary: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut in_timeline = false;

    for line in format_compact_summary(summary).lines() {
        let trimmed = line.trim_end();
        if trimmed == "- Key timeline:" {
            in_timeline = true;
            continue;
        }
        if !in_timeline {
            continue;
        }
        if trimmed.is_empty() {
            break;
        }
        lines.push(trimmed.to_string());
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::{
        apply_model_compact_summary, build_model_compact_request, collect_key_files,
        compact_session, estimate_session_tokens, estimate_tokens, format_compact_summary,
        get_compact_continuation_message, infer_pending_work, should_compact,
        should_compact_for_model, CompactionConfig,
    };
    use crate::model_context::context_window_for_model;
    use crate::session::{ContentBlock, ConversationMessage, MessageRole, Session};

    #[test]
    fn formats_compact_summary_like_upstream() {
        let summary = "<analysis>scratch</analysis>\n<summary>Kept work</summary>";
        assert_eq!(format_compact_summary(summary), "Summary:\nKept work");
    }

    #[test]
    fn leaves_small_sessions_unchanged() {
        let session = Session {
            messages: vec![ConversationMessage::user_text("hello")],
            ..Session::new()
        };

        let result = compact_session(&session, CompactionConfig::default());
        assert_eq!(result.removed_message_count, 0);
        assert_eq!(result.compacted_session, session);
        assert!(result.summary.is_empty());
        assert!(result.formatted_summary.is_empty());
    }

    #[test]
    fn compacts_older_messages_into_a_system_summary() {
        let session = Session {
            messages: vec![
                ConversationMessage::user_text("one ".repeat(200)),
                ConversationMessage::assistant(vec![ContentBlock::Text {
                    text: "two ".repeat(200),
                }]),
                ConversationMessage::tool_result("1", "bash", "ok ".repeat(200), false),
                ConversationMessage {
                    role: MessageRole::Assistant,
                    blocks: vec![ContentBlock::Text {
                        text: "recent".to_string(),
                    }],
                    usage: None,
                },
            ],
            ..Session::new()
        };

        let result = compact_session(
            &session,
            CompactionConfig {
                preserve_recent_messages: 2,
                max_estimated_tokens: 1,
            },
        );

        assert_eq!(result.removed_message_count, 2);
        assert_eq!(
            result.compacted_session.messages[0].role,
            MessageRole::System
        );
        assert!(matches!(
            &result.compacted_session.messages[0].blocks[0],
            ContentBlock::Text { text } if text.contains("Summary:")
        ));
        assert!(result.formatted_summary.contains("Scope:"));
        assert!(result.formatted_summary.contains("Key timeline:"));
        assert!(should_compact(
            &session,
            CompactionConfig {
                preserve_recent_messages: 2,
                max_estimated_tokens: 1,
            }
        ));
        assert!(
            estimate_session_tokens(&result.compacted_session) < estimate_session_tokens(&session)
        );
    }

    #[test]
    fn keeps_previous_compacted_context_when_compacting_again() {
        let initial_session = Session {
            messages: vec![
                ConversationMessage::user_text("Investigate rust/crates/runtime/src/compact.rs"),
                ConversationMessage::assistant(vec![ContentBlock::Text {
                    text: "I will inspect the compact flow.".to_string(),
                }]),
                ConversationMessage::user_text(
                    "Also update rust/crates/runtime/src/conversation.rs",
                ),
                ConversationMessage::assistant(vec![ContentBlock::Text {
                    text: "Next: preserve prior summary context during auto compact.".to_string(),
                }]),
            ],
            ..Session::new()
        };
        let config = CompactionConfig {
            preserve_recent_messages: 2,
            max_estimated_tokens: 1,
        };

        let first = compact_session(&initial_session, config);
        let mut follow_up_messages = first.compacted_session.messages.clone();
        follow_up_messages.extend([
            ConversationMessage::user_text("Please add regression tests for compaction."),
            ConversationMessage::assistant(vec![ContentBlock::Text {
                text: "Working on regression coverage now.".to_string(),
            }]),
        ]);

        let second = compact_session(
            &Session {
                messages: follow_up_messages,
                ..Session::new()
            },
            config,
        );

        assert!(second
            .formatted_summary
            .contains("Previously compacted context:"));
        assert!(second
            .formatted_summary
            .contains("Scope: 2 earlier messages compacted"));
        assert!(second
            .formatted_summary
            .contains("Newly compacted context:"));
        assert!(second
            .formatted_summary
            .contains("Also update rust/crates/runtime/src/conversation.rs"));
        assert!(matches!(
            &second.compacted_session.messages[0].blocks[0],
            ContentBlock::Text { text }
                if text.contains("Previously compacted context:")
                    && text.contains("Newly compacted context:")
        ));
        assert!(matches!(
            &second.compacted_session.messages[1].blocks[0],
            ContentBlock::Text { text } if text.contains("Please add regression tests for compaction.")
        ));
    }

    #[test]
    fn ignores_existing_compacted_summary_when_deciding_to_recompact() {
        let summary = "<summary>Conversation summary:\n- Scope: earlier work preserved.\n- Key timeline:\n  - user: large preserved context\n</summary>";
        let session = Session {
            messages: vec![
                ConversationMessage {
                    role: MessageRole::System,
                    blocks: vec![ContentBlock::Text {
                        text: get_compact_continuation_message(summary, true, true),
                    }],
                    usage: None,
                },
                ConversationMessage::user_text("tiny"),
                ConversationMessage::assistant(vec![ContentBlock::Text {
                    text: "recent".to_string(),
                }]),
            ],
            ..Session::new()
        };

        assert!(!should_compact(
            &session,
            CompactionConfig {
                preserve_recent_messages: 2,
                max_estimated_tokens: 1,
            }
        ));
    }

    #[test]
    fn truncates_long_blocks_in_summary() {
        let summary = super::summarize_block(&ContentBlock::Text {
            text: "x".repeat(400),
        });
        assert!(summary.ends_with('…'));
        assert!(summary.chars().count() <= 161);
    }

    #[test]
    fn extracts_key_files_from_message_content() {
        let files = collect_key_files(&[ConversationMessage::user_text(
            "Update rust/crates/runtime/src/compact.rs and rust/crates/tools/src/lib.rs next.",
        )]);
        assert!(files.contains(&"rust/crates/runtime/src/compact.rs".to_string()));
        assert!(files.contains(&"rust/crates/tools/src/lib.rs".to_string()));
    }

    #[test]
    fn infers_pending_work_from_recent_messages() {
        let pending = infer_pending_work(&[
            ConversationMessage::user_text("done"),
            ConversationMessage::assistant(vec![ContentBlock::Text {
                text: "Next: update tests and follow up on remaining CLI polish.".to_string(),
            }]),
        ]);
        assert_eq!(pending.len(), 1);
        assert!(pending[0].contains("Next: update tests"));
    }

    #[test]
    fn estimate_tokens_ascii_uses_byte_heuristic() {
        let text = "a".repeat(400);
        assert_eq!(estimate_tokens(&text), 101);
    }

    #[test]
    fn estimate_tokens_cjk_uses_char_heuristic() {
        // Three UTF-8 bytes per char → bytes/chars ratio > 1.5
        let text = "字".repeat(100);
        assert_eq!(text.chars().count(), 100);
        let t = estimate_tokens(&text);
        assert!(
            t < 200,
            "CJK path should estimate fewer tokens than byte/4 for same char count"
        );
        assert_eq!(t, 68);
    }

    #[test]
    fn should_compact_for_model_false_when_under_budget() {
        let session = Session {
            messages: vec![ConversationMessage::user_text("hi")],
            ..Session::new()
        };
        assert!(!should_compact_for_model(&session, "unknown-small-context"));
    }

    #[test]
    fn should_compact_for_model_true_when_at_or_over_effective_budget() {
        let budget = context_window_for_model("unknown-small-context").effective_input_budget();
        // ASCII: `estimate_tokens` is `byte_len / 4 + 1` — size so sum meets `budget`.
        let filler_len = budget.saturating_sub(1).saturating_mul(4);
        let text = "x".repeat(filler_len);
        let session = Session {
            messages: vec![ConversationMessage::user_text(text)],
            ..Session::new()
        };
        assert!(estimate_session_tokens(&session) >= budget);
        assert!(should_compact_for_model(&session, "unknown-small-context"));
    }

    #[test]
    fn build_model_compact_request_includes_removed_segment_and_preserve_index() {
        let session = Session {
            messages: vec![
                ConversationMessage::user_text("one ".repeat(200)),
                ConversationMessage::assistant(vec![ContentBlock::Text {
                    text: "two ".repeat(200),
                }]),
                ConversationMessage::tool_result("1", "bash", "ok ".repeat(200), false),
                ConversationMessage {
                    role: MessageRole::Assistant,
                    blocks: vec![ContentBlock::Text {
                        text: "recent".to_string(),
                    }],
                    usage: None,
                },
            ],
            ..Session::new()
        };
        let config = CompactionConfig {
            preserve_recent_messages: 2,
            max_estimated_tokens: 1,
        };
        let (messages, preserve_from) =
            build_model_compact_request(&session, &config).expect("should compact");
        assert_eq!(preserve_from, 2);
        assert_eq!(messages.len(), 1);
        let ContentBlock::Text { text } = &messages[0].blocks[0] else {
            panic!("expected user text");
        };
        assert!(text.contains("## Message 0"));
        assert!(text.contains("## Message 1"));
        assert!(!text.contains("recent"));
    }

    #[test]
    fn apply_model_compact_summary_matches_compact_session_shape() {
        let session = Session {
            messages: vec![
                ConversationMessage::user_text("old ".repeat(100)),
                ConversationMessage::assistant(vec![ContentBlock::Text {
                    text: "reply".to_string(),
                }]),
                ConversationMessage::user_text("keep me"),
            ],
            ..Session::new()
        };
        let config = CompactionConfig {
            preserve_recent_messages: 1,
            max_estimated_tokens: 1,
        };
        let (_, preserve_from) = build_model_compact_request(&session, &config).unwrap();
        let model_summary = "User asked for work; assistant replied; pending: tests.";
        let applied = apply_model_compact_summary(&session, model_summary, preserve_from);
        assert_eq!(applied.removed_message_count, 2);
        assert_eq!(applied.compacted_session.messages.len(), 2);
        assert_eq!(
            applied.compacted_session.messages[1].role,
            MessageRole::User
        );
        assert!(applied.summary.contains(model_summary));
    }

    #[test]
    fn build_model_compact_request_returns_none_when_not_needed() {
        let session = Session {
            messages: vec![ConversationMessage::user_text("hello")],
            ..Session::new()
        };
        assert!(build_model_compact_request(&session, &CompactionConfig::default()).is_none());
    }
}
