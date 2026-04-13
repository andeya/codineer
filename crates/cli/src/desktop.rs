//! Tauri / desktop integration: single-turn chat streaming and agent turns
//! using the same provider and runtime stack as the CLI REPL.

use std::fmt::Write;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use aineer_engine::{
    assistant_text_from_stream_events, ContentBlock, ConversationMessage, PermissionMode, Session,
    TurnSummary,
};

pub use crate::runtime_client::DesktopStreamDelta;

use crate::bootstrap::{build_runtime_plugin_state_for_cwd, build_system_prompt_for_cwd};
use crate::cli::create_mcp_manager;
use crate::error::CliError;
use crate::max_tokens_for_model;
use crate::runtime_client::{
    build_runtime, convert_messages, stream_with_client_deltas, DesktopStreamHook, ModelResolver,
    RuntimeParams,
};

pub use crate::runtime_client::StreamDelta;

/// One shell command + captured output for desktop AI context.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShellContextSnippet {
    pub command: String,
    pub output: String,
}

/// One turn in the desktop chat history (OpenAI-style roles).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatHistoryTurn {
    pub role: String,
    pub content: String,
}

fn chat_turns_to_messages(turns: &[ChatHistoryTurn]) -> Vec<ConversationMessage> {
    turns
        .iter()
        .filter_map(|t| {
            let body = t.content.trim();
            if body.is_empty() {
                return None;
            }
            match t.role.to_lowercase().as_str() {
                "user" => Some(ConversationMessage::user_text(body)),
                "assistant" => Some(ConversationMessage::assistant(vec![ContentBlock::Text {
                    text: body.to_string(),
                }])),
                _ => None,
            }
        })
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum DesktopStreamError {
    #[error(transparent)]
    Cli(#[from] CliError),
    #[error(transparent)]
    Runtime(#[from] aineer_engine::RuntimeError),
}

/// Max characters per shell snippet output before truncation.
const SHELL_SNIPPET_MAX_CHARS: usize = 2000;

fn user_message_with_shell_context(message: &str, shell_context: &[ShellContextSnippet]) -> String {
    if shell_context.is_empty() {
        return message.to_string();
    }
    // TODO: inject as a separate system message instead of prepending to user text
    let mut body = String::from("## Recent terminal output (context)\n\n");
    for snip in shell_context {
        body.push_str("```\n$ ");
        body.push_str(&snip.command);
        body.push('\n');
        if snip.output.len() > SHELL_SNIPPET_MAX_CHARS {
            body.push_str(&snip.output[..SHELL_SNIPPET_MAX_CHARS]);
            body.push_str("\n... (truncated)");
        } else {
            body.push_str(&snip.output);
        }
        body.push_str("\n```\n\n");
    }
    body.push_str(message);
    body
}

fn format_turn_summary(summary: &TurnSummary) -> String {
    let mut out = String::new();
    for msg in &summary.assistant_messages {
        for block in &msg.blocks {
            match block {
                ContentBlock::Text { text } if !text.is_empty() => {
                    out.push_str(text);
                    out.push_str("\n\n");
                }
                ContentBlock::ToolUse { name, input, .. } => {
                    let _ = writeln!(out, "### Tool `{name}`\n```json\n{input}\n```\n");
                }
                _ => {}
            }
        }
    }
    out.trim().to_string()
}

/// Resolve model string and load config+registry for `cwd` (shared by chat + agent).
fn resolve_model_for_cwd(
    cwd: &Path,
    model_override: Option<&str>,
) -> Result<
    (
        aineer_engine::RuntimeConfig,
        aineer_tools::GlobalToolRegistry,
        String,
    ),
    DesktopStreamError,
> {
    let (runtime_config, tool_registry) = build_runtime_plugin_state_for_cwd(cwd)?;
    let model_to_resolve = model_override
        .map(String::from)
        .or_else(|| runtime_config.model().map(std::string::ToString::to_string))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "auto".to_string());
    Ok((runtime_config, tool_registry, model_to_resolve))
}

/// Stream one user message (no tools) and invoke `on_delta` for each text/thinking chunk.
/// Returns the full assistant plain text assembled from stream events.
pub async fn stream_desktop_chat(
    cwd: &Path,
    model_override: Option<&str>,
    message: &str,
    shell_context: &[ShellContextSnippet],
    prior_turns: &[ChatHistoryTurn],
    cancel: Arc<AtomicBool>,
    on_delta: impl FnMut(StreamDelta<'_>),
) -> Result<String, DesktopStreamError> {
    let (runtime_config, _tool_registry, model_to_resolve) =
        resolve_model_for_cwd(cwd, model_override)?;
    let resolver = ModelResolver::new(&runtime_config);
    let resolved = resolver.resolve(&model_to_resolve)?;

    let system_prompt = build_system_prompt_for_cwd(cwd)?;
    let user_text = user_message_with_shell_context(message, shell_context);
    let mut conv_msgs = chat_turns_to_messages(prior_turns);
    conv_msgs.push(ConversationMessage::user_blocks(vec![ContentBlock::Text {
        text: user_text,
    }]));
    let api_messages = convert_messages(&conv_msgs);

    let request = aineer_api::MessageRequest {
        model: resolved.model.clone(),
        max_tokens: max_tokens_for_model(&resolved.model),
        messages: api_messages,
        system: Some(system_prompt),
        tools: None,
        tool_choice: None,
        stream: true,
        thinking: None,
        gemini_cached_content: None,
    };

    let events = stream_with_client_deltas(&resolved.client, &request, cancel, on_delta).await?;
    Ok(assistant_text_from_stream_events(events))
}

/// Run one agent turn with tools enabled. Uses `PermissionMode::Allow` so the GUI does not block
/// on stdin prompts (GUI approval flow is TODO).
///
/// `on_delta` receives each streamed text / thinking chunk (same semantics as desktop chat).
pub fn run_desktop_agent_turn(
    cwd: &Path,
    model_override: Option<&str>,
    goal: &str,
    shell_context: &[ShellContextSnippet],
    prior_turns: &[ChatHistoryTurn],
    cancel: Arc<AtomicBool>,
    on_delta: Box<dyn FnMut(DesktopStreamDelta) + Send>,
) -> Result<String, DesktopStreamError> {
    let (runtime_config, tool_registry, model_to_resolve) =
        resolve_model_for_cwd(cwd, model_override)?;
    let system_prompt = build_system_prompt_for_cwd(cwd)?;
    let goal_text = user_message_with_shell_context(goal, shell_context);
    let mcp_manager = create_mcp_manager();

    let mut session = Session::new();
    session.messages = chat_turns_to_messages(prior_turns);

    let hook: DesktopStreamHook = Arc::new(Mutex::new(on_delta));

    let build = build_runtime(RuntimeParams {
        session,
        model: model_to_resolve,
        system_prompt,
        enable_tools: true,
        emit_output: false,
        allowed_tools: None,
        permission_mode: PermissionMode::Allow,
        progress_reporter: None,
        mcp_manager,
        preloaded_state: Some((runtime_config, tool_registry)),
        desktop_stream_hook: Some(hook),
        stream_cancel: Some(cancel),
    })?;

    let mut runtime = build.runtime;
    let summary = runtime.run_turn(goal_text, None)?;
    Ok(format_turn_summary(&summary))
}
