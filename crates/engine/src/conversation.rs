use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub use protocol::error::RuntimeError;

use protocol::events::RuntimeEvent;
use protocol::observer::RuntimeObserver;
use protocol::prompt_types::SystemBlock;

use crate::compact::{
    apply_model_compact_summary, build_model_compact_request, compact_session,
    estimate_session_tokens, format_compact_summary, CompactionConfig, CompactionResult,
    ModelCompactionConfig, ModelCompactionResult,
};
use crate::permissions::{PermissionOutcome, PermissionPolicy, PermissionPrompter};
use crate::recovery::{preserve_recent_for_reactive_compact, RecoveryEngine, RecoveryStrategy};
use crate::session::{ContentBlock, ConversationMessage, Session};
use crate::usage::{TokenUsage, UsageTracker};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApiRequest {
    pub system_prompt: Vec<SystemBlock>,
    pub messages: Vec<ConversationMessage>,
    /// When set, clients that support it use this model for this request only.
    pub model_override: Option<String>,
    /// When set, caps the model output budget for this request (e.g. compaction summary).
    pub max_output_tokens: Option<u32>,
}

/// Concatenate assistant text deltas from a streamed response.
#[must_use]
pub fn assistant_text_from_stream_events(events: Vec<AssistantEvent>) -> String {
    events
        .into_iter()
        .filter_map(|event| match event {
            AssistantEvent::TextDelta(text) => Some(text),
            _ => None,
        })
        .collect()
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantEvent {
    TextDelta(String),
    ToolUse {
        id: String,
        name: String,
        input: String,
    },
    Usage(TokenUsage),
    MessageStop,
}

pub trait ApiClient: Send + Sync {
    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError>;

    /// Returns the model name currently being used for requests.
    /// This may differ from the originally configured model after a fallback.
    fn active_model(&self) -> &str;
}

pub trait ToolExecutor: Send + Sync {
    fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, ToolError>;

    fn execute_with_progress(
        &mut self,
        tool_name: &str,
        input: &str,
        _on_progress: Option<&mut dyn FnMut(&str)>,
    ) -> Result<String, ToolError> {
        self.execute(tool_name, input)
    }

    /// Returns `true` if `tool_name` can safely run concurrently with other tools
    /// (i.e. it is read-only and has no ordering dependencies).
    /// Defaults to `false` (conservative: sequential execution).
    fn is_concurrency_safe(&self, _tool_name: &str) -> bool {
        false
    }

    /// Execute a batch of concurrency-safe tools in parallel.
    ///
    /// Each element of `calls` is `(tool_name, json_input)`. The return vec has
    /// the same length and order as `calls`.
    ///
    /// Returns `None` when parallel execution is not supported by this executor,
    /// in which case the caller falls back to sequential `execute` calls.
    fn execute_batch(&self, _calls: &[(&str, &str)]) -> Option<Vec<Result<String, ToolError>>> {
        None
    }

    /// Like [`Self::execute_batch`], but receives an abort flag other tools in the
    /// same batch can observe when a sibling (typically `bash`) fails.
    ///
    /// Default implementation ignores `abort_flag` and delegates to [`Self::execute_batch`].
    fn execute_batch_with_abort(
        &self,
        calls: &[(&str, &str)],
        abort_flag: &Arc<AtomicBool>,
    ) -> Option<Vec<Result<String, ToolError>>> {
        let _ = abort_flag;
        self.execute_batch(calls)
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolErrorCode {
    InvalidInput,
    PermissionDenied,
    NotFound,
    Conflict,
    Timeout,
    InternalError,
}

impl Display for ToolErrorCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput => f.write_str("invalid_input"),
            Self::PermissionDenied => f.write_str("permission_denied"),
            Self::NotFound => f.write_str("not_found"),
            Self::Conflict => f.write_str("conflict"),
            Self::Timeout => f.write_str("timeout"),
            Self::InternalError => f.write_str("internal_error"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError {
    code: ToolErrorCode,
    message: String,
}

impl ToolError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            code: ToolErrorCode::InternalError,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn with_code(code: ToolErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn code(&self) -> ToolErrorCode {
        self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for ToolError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ToolError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnSummary {
    pub assistant_messages: Vec<ConversationMessage>,
    pub tool_results: Vec<ConversationMessage>,
    pub iterations: usize,
    pub usage: TokenUsage,
}

pub struct ConversationRuntime<C, T, O: RuntimeObserver = ()> {
    session: Session,
    session_path: Option<PathBuf>,
    api_client: C,
    tool_executor: T,
    permission_policy: PermissionPolicy,
    system_prompt: Vec<SystemBlock>,
    model_compaction: ModelCompactionConfig,
    max_iterations: usize,
    usage_tracker: UsageTracker,
    recovery: RecoveryEngine,
    observer: O,
}

impl<C, T, O> ConversationRuntime<C, T, O>
where
    C: ApiClient,
    T: ToolExecutor,
    O: RuntimeObserver,
{
    #[must_use]
    pub fn new(
        session: Session,
        api_client: C,
        tool_executor: T,
        permission_policy: PermissionPolicy,
        system_prompt: Vec<SystemBlock>,
        observer: O,
    ) -> Self {
        let usage_tracker = UsageTracker::from_session(&session);
        Self {
            session,
            session_path: None,
            api_client,
            tool_executor,
            permission_policy,
            system_prompt,
            model_compaction: ModelCompactionConfig::default(),
            max_iterations: 200,
            usage_tracker,
            recovery: RecoveryEngine::new(true),
            observer,
        }
    }

    #[must_use]
    pub fn with_model_compaction(mut self, config: ModelCompactionConfig) -> Self {
        self.model_compaction = config;
        self
    }

    #[must_use]
    pub fn with_session_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.session_path = Some(path.into());
        self
    }

    fn persist_session(&self) {
        let Some(path) = &self.session_path else {
            return;
        };
        if let Err(e) = self.session.save_to_path(path) {
            eprintln!(
                "aineer-runtime: failed to persist session to {}: {e}",
                path.display()
            );
        }
    }

    pub fn update_system_prompt(&mut self, system_prompt: Vec<SystemBlock>) {
        self.system_prompt = system_prompt;
    }

    #[must_use]
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    pub fn run_turn(
        &mut self,
        user_input: impl Into<String>,
        prompter: Option<&mut dyn PermissionPrompter>,
    ) -> Result<TurnSummary, RuntimeError> {
        let blocks = vec![ContentBlock::Text {
            text: user_input.into(),
        }];
        self.run_turn_with_blocks(blocks, prompter)
    }

    pub fn run_turn_with_blocks(
        &mut self,
        blocks: Vec<ContentBlock>,
        mut prompter: Option<&mut dyn PermissionPrompter>,
    ) -> Result<TurnSummary, RuntimeError> {
        self.session
            .messages
            .push(ConversationMessage::user_blocks(blocks));
        self.persist_session();

        let mut assistant_messages = Vec::new();
        let mut tool_results = Vec::new();
        let mut iterations = 0;

        loop {
            iterations += 1;
            if iterations > self.max_iterations {
                let _ = self.observer.on_event(&RuntimeEvent::Stop {
                    reason: "max iterations exceeded".into(),
                });
                return Err(RuntimeError::MaxIterations {
                    iterations: self.max_iterations,
                });
            }

            let _ = self.observer.on_event(&RuntimeEvent::TurnStart {
                iteration: iterations,
                turn: iterations,
            });

            let events = self.stream_with_recovery()?;
            let (assistant_message, usage) = build_assistant_message(events)?;
            if let Some(usage) = usage {
                self.usage_tracker.record(usage);
            }

            let pending_tool_uses = extract_tool_uses(&assistant_message);
            self.session.messages.push(assistant_message.clone());
            assistant_messages.push(assistant_message);

            if pending_tool_uses.is_empty() {
                let _ = self.observer.on_event(&RuntimeEvent::TurnEnd {
                    iteration: iterations,
                    turn: iterations,
                });
                break;
            }

            let slot_status = self.check_permissions(&pending_tool_uses, &mut prompter);
            let exec_results = self.execute_tools(&pending_tool_uses, &slot_status);
            let messages = self.apply_post_hooks(pending_tool_uses, slot_status, exec_results)?;
            for msg in messages {
                self.session.messages.push(msg.clone());
                tool_results.push(msg);
            }
        }

        Ok(TurnSummary {
            assistant_messages,
            tool_results,
            iterations,
            usage: self.usage_tracker.cumulative_usage(),
        })
    }

    /// Send API request with automatic recovery on transient failures.
    ///
    /// When the recovery engine selects `AutocompactRetry` or `ReactiveCompactRetry`,
    /// this method actually compacts `self.session` before retrying — not a blind retry.
    fn stream_with_recovery(&mut self) -> Result<Vec<AssistantEvent>, RuntimeError> {
        loop {
            let request = ApiRequest {
                system_prompt: self.system_prompt.clone(),
                messages: self.session.messages.clone(),
                model_override: None,
                max_output_tokens: None,
            };
            match self.api_client.stream(request) {
                Ok(events) => {
                    self.recovery.reset();
                    return Ok(events);
                }
                Err(RuntimeError::Api {
                    status_code,
                    ref error_type,
                    ref message,
                }) => {
                    let kind =
                        self.recovery
                            .classify(status_code, error_type.as_deref(), Some(message));
                    let strategy = self.recovery.select_strategy(&kind);
                    self.recovery.record_attempt(&strategy);
                    match strategy {
                        RecoveryStrategy::Retry { .. } | RecoveryStrategy::StreamingFallback => {
                            continue
                        }
                        RecoveryStrategy::AutocompactRetry => {
                            let config = CompactionConfig::default();
                            let _ = self.observer.on_event(&RuntimeEvent::PreCompact {
                                strategy: "autocompact",
                                message_count: self.session.messages.len(),
                            });
                            let result = self.compact_for_autorecovery(config);
                            if result.removed_message_count > 0 {
                                let _ = self.observer.on_event(&RuntimeEvent::PostCompact {
                                    strategy: "autocompact",
                                    messages_removed: result.removed_message_count,
                                });
                                self.session = result.compacted_session;
                            }
                            continue;
                        }
                        RecoveryStrategy::ReactiveCompactRetry => {
                            let preserve = preserve_recent_for_reactive_compact(
                                Some(message.as_str()),
                                self.session.messages.len(),
                            );
                            let config = CompactionConfig {
                                preserve_recent_messages: preserve,
                                max_estimated_tokens: 1,
                            };
                            let _ = self.observer.on_event(&RuntimeEvent::PreCompact {
                                strategy: "reactive",
                                message_count: self.session.messages.len(),
                            });
                            let result = compact_session(&self.session, config);
                            if result.removed_message_count > 0 {
                                let _ = self.observer.on_event(&RuntimeEvent::PostCompact {
                                    strategy: "reactive",
                                    messages_removed: result.removed_message_count,
                                });
                                self.session = result.compacted_session;
                            }
                            continue;
                        }
                        RecoveryStrategy::GiveUp { reason } => {
                            return Err(RuntimeError::RecoveryExhausted {
                                kind: format!("{kind:?}"),
                                reason,
                            });
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Run permission checks and pre-hook observers for each pending tool use.
    fn check_permissions(
        &mut self,
        pending: &[(String, String, String)],
        prompter: &mut Option<&mut dyn PermissionPrompter>,
    ) -> Vec<Result<Vec<String>, ConversationMessage>> {
        let mut slots = Vec::with_capacity(pending.len());
        for (tool_use_id, tool_name, input) in pending {
            let outcome = if let Some(prompt) = prompter.as_mut() {
                self.permission_policy
                    .authorize(tool_name, input, Some(*prompt))
            } else {
                self.permission_policy.authorize(tool_name, input, None)
            };
            match outcome {
                PermissionOutcome::Deny { reason } => {
                    slots.push(Err(ConversationMessage::tool_result(
                        tool_use_id.clone(),
                        tool_name.clone(),
                        reason,
                        true,
                    )));
                }
                PermissionOutcome::Allow => {
                    let directive = self.observer.on_event(&RuntimeEvent::PreToolUse {
                        tool_name,
                        tool_use_id,
                        input,
                    });
                    if directive.is_denied() {
                        let deny_msg = directive
                            .deny_reason()
                            .unwrap_or("hook denied tool")
                            .to_string();
                        slots.push(Err(ConversationMessage::tool_result(
                            tool_use_id.clone(),
                            tool_name.clone(),
                            deny_msg,
                            true,
                        )));
                    } else {
                        slots.push(Ok(directive.messages));
                    }
                }
            }
        }
        slots
    }

    /// Execute approved tools (concurrently when safe, otherwise sequentially).
    fn execute_tools(
        &mut self,
        pending: &[(String, String, String)],
        slots: &[Result<Vec<String>, ConversationMessage>],
    ) -> BTreeMap<usize, Result<String, ToolError>> {
        let ready: Vec<usize> = slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.is_ok().then_some(i))
            .collect();

        let outputs = self.run_tool_batch(pending, &ready);
        ready.into_iter().zip(outputs).collect()
    }

    fn run_tool_batch(
        &mut self,
        pending: &[(String, String, String)],
        ready: &[usize],
    ) -> Vec<Result<String, ToolError>> {
        let try_concurrent = ready.len() > 1
            && ready
                .iter()
                .all(|&i| self.tool_executor.is_concurrency_safe(&pending[i].1));

        if try_concurrent {
            let calls: Vec<(&str, &str)> = ready
                .iter()
                .map(|&i| (pending[i].1.as_str(), pending[i].2.as_str()))
                .collect();
            if let Some(batch) = self.tool_executor.execute_batch(&calls) {
                return batch;
            }
        }
        ready
            .iter()
            .map(|&i| {
                self.tool_executor
                    .execute_with_progress(&pending[i].1, &pending[i].2, None)
            })
            .collect()
    }

    /// Run post-hooks and build result messages for the session.
    fn apply_post_hooks(
        &mut self,
        pending: Vec<(String, String, String)>,
        slots: Vec<Result<Vec<String>, ConversationMessage>>,
        mut exec_results: BTreeMap<usize, Result<String, ToolError>>,
    ) -> Result<Vec<ConversationMessage>, RuntimeError> {
        let mut results = Vec::with_capacity(pending.len());
        for (i, ((tool_use_id, tool_name, _input), slot)) in
            pending.into_iter().zip(slots).enumerate()
        {
            let msg = match slot {
                Err(resolved) => resolved,
                Ok(pre_messages) => {
                    let exec_result = exec_results.remove(&i).ok_or_else(|| {
                        RuntimeError::new("missing execution result for approved tool slot")
                    })?;
                    let (mut output, mut is_error) = match exec_result {
                        Ok(o) => (o, false),
                        Err(e) => (e.to_string(), true),
                    };
                    output = merge_hook_feedback(&pre_messages, output, false);

                    let post_directive = if is_error {
                        self.observer.on_event(&RuntimeEvent::PostToolUseFailure {
                            tool_name: &tool_name,
                            tool_use_id: &tool_use_id,
                            error: &output,
                        })
                    } else {
                        self.observer.on_event(&RuntimeEvent::PostToolUse {
                            tool_name: &tool_name,
                            tool_use_id: &tool_use_id,
                            output: &output,
                            is_error: false,
                        })
                    };
                    if post_directive.is_denied() {
                        is_error = true;
                    }
                    output = merge_hook_feedback(
                        &post_directive.messages,
                        output,
                        post_directive.is_denied(),
                    );

                    ConversationMessage::tool_result(tool_use_id, tool_name, output, is_error)
                }
            };
            results.push(msg);
        }
        Ok(results)
    }

    /// Model-assisted compaction: summarizes removable history via [`ApiClient::stream`], then applies
    /// [`apply_model_compact_summary`]. Requires [`ModelCompactionConfig::enabled`].
    pub fn compact_with_model(&mut self) -> Result<ModelCompactionResult, RuntimeError> {
        if !self.model_compaction.enabled {
            return Err(RuntimeError::new(
                "model compaction is disabled; set ModelCompactionConfig.enabled",
            ));
        }
        let config = CompactionConfig::default();
        let Some((messages, preserve_from)) = build_model_compact_request(&self.session, &config)
        else {
            return Err(RuntimeError::new("nothing to compact"));
        };
        let mc = &self.model_compaction;
        let max_out = u32::try_from(mc.max_summary_tokens).unwrap_or(u32::MAX);
        let request = ApiRequest {
            system_prompt: vec![SystemBlock::text(mc.summary_prompt.clone())],
            messages,
            model_override: None,
            max_output_tokens: Some(max_out),
        };
        let events = self.api_client.stream(request)?;
        let text = assistant_text_from_stream_events(events);
        if text.trim().is_empty() {
            return Err(RuntimeError::new("model returned empty compaction summary"));
        }
        let result = apply_model_compact_summary(&self.session, &text, preserve_from);
        self.session = result.compacted_session.clone();
        self.persist_session();
        Ok(result)
    }

    fn compact_for_autorecovery(&mut self, config: CompactionConfig) -> CompactionResult {
        if self.model_compaction.enabled {
            if let Some((messages, preserve_from)) =
                build_model_compact_request(&self.session, &config)
            {
                let mc = &self.model_compaction;
                let max_out = u32::try_from(mc.max_summary_tokens).unwrap_or(u32::MAX);
                let request = ApiRequest {
                    system_prompt: vec![SystemBlock::text(mc.summary_prompt.clone())],
                    messages,
                    model_override: None,
                    max_output_tokens: Some(max_out),
                };
                if let Ok(events) = self.api_client.stream(request) {
                    let text = assistant_text_from_stream_events(events);
                    if !text.trim().is_empty() {
                        let applied =
                            apply_model_compact_summary(&self.session, &text, preserve_from);
                        return CompactionResult {
                            summary: applied.summary.clone(),
                            formatted_summary: format_compact_summary(&applied.summary),
                            compacted_session: applied.compacted_session,
                            removed_message_count: applied.removed_message_count,
                        };
                    }
                }
            }
        }
        compact_session(&self.session, config)
    }

    #[must_use]
    pub fn compact(&self, config: CompactionConfig) -> CompactionResult {
        compact_session(&self.session, config)
    }

    #[must_use]
    pub fn estimated_tokens(&self) -> usize {
        estimate_session_tokens(&self.session)
    }

    #[must_use]
    pub fn usage(&self) -> &UsageTracker {
        &self.usage_tracker
    }

    #[must_use]
    pub fn active_model(&self) -> &str {
        self.api_client.active_model()
    }

    #[must_use]
    pub fn session(&self) -> &Session {
        &self.session
    }

    #[must_use]
    pub fn into_session(self) -> Session {
        self.session
    }
}

fn extract_tool_uses(msg: &ConversationMessage) -> Vec<(String, String, String)> {
    msg.blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolUse { id, name, input } => {
                Some((id.clone(), name.clone(), input.clone()))
            }
            _ => None,
        })
        .collect()
}

fn build_assistant_message(
    events: Vec<AssistantEvent>,
) -> Result<(ConversationMessage, Option<TokenUsage>), RuntimeError> {
    let mut text = String::new();
    let mut blocks = Vec::new();
    let mut finished = false;
    let mut usage = None;

    for event in events {
        match event {
            AssistantEvent::TextDelta(delta) => text.push_str(&delta),
            AssistantEvent::ToolUse { id, name, input } => {
                flush_text_block(&mut text, &mut blocks);
                blocks.push(ContentBlock::ToolUse { id, name, input });
            }
            AssistantEvent::Usage(value) => usage = Some(value),
            AssistantEvent::MessageStop => {
                finished = true;
            }
        }
    }

    flush_text_block(&mut text, &mut blocks);

    if !finished {
        return Err(RuntimeError::IncompleteStream);
    }
    if blocks.is_empty() {
        return Err(RuntimeError::EmptyReply);
    }

    Ok((
        ConversationMessage::assistant_with_usage(blocks, usage),
        usage,
    ))
}

fn flush_text_block(text: &mut String, blocks: &mut Vec<ContentBlock>) {
    if !text.is_empty() {
        blocks.push(ContentBlock::Text {
            text: std::mem::take(text),
        });
    }
}

fn merge_hook_feedback(messages: &[String], output: String, denied: bool) -> String {
    if messages.is_empty() {
        return output;
    }

    let mut sections = Vec::new();
    if !output.trim().is_empty() {
        sections.push(output);
    }
    let label = if denied {
        "Hook feedback (denied)"
    } else {
        "Hook feedback"
    };
    sections.push(format!("{label}:\n{}", messages.join("\n")));
    sections.join("\n\n")
}

type ToolHandler = Box<dyn Fn(&str) -> Result<String, ToolError> + Send + Sync>;

#[derive(Default)]
pub struct StaticToolExecutor {
    handlers: BTreeMap<String, ToolHandler>,
}

impl StaticToolExecutor {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn register(
        mut self,
        tool_name: impl Into<String>,
        handler: impl Fn(&str) -> Result<String, ToolError> + Send + Sync + 'static,
    ) -> Self {
        self.handlers.insert(tool_name.into(), Box::new(handler));
        self
    }
}

impl ToolExecutor for StaticToolExecutor {
    fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, ToolError> {
        self.handlers
            .get(tool_name)
            .ok_or_else(|| ToolError::new(format!("unknown tool: {tool_name}")))?(input)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ApiClient, ApiRequest, AssistantEvent, ConversationRuntime, RuntimeError,
        StaticToolExecutor,
    };
    use crate::compact::{CompactionConfig, ModelCompactionConfig};
    #[cfg(unix)]
    use crate::config::RuntimeHookConfig;
    #[cfg(unix)]
    use crate::hooks::HookDispatcher;
    use crate::permissions::{
        PermissionMode, PermissionPolicy, PermissionPromptDecision, PermissionPrompter,
        PermissionRequest,
    };
    use crate::prompt::{ProjectContext, SystemPromptBuilder};
    use crate::session::{ContentBlock, ConversationMessage, MessageRole, Session};
    use crate::usage::TokenUsage;
    use protocol::prompt_types::SystemBlock;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct ScriptedApiClient {
        call_count: usize,
    }

    impl ApiClient for ScriptedApiClient {
        fn active_model(&self) -> &str {
            "test-model"
        }

        fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
            self.call_count += 1;
            match self.call_count {
                1 => {
                    assert!(request
                        .messages
                        .iter()
                        .any(|message| message.role == MessageRole::User));
                    Ok(vec![
                        AssistantEvent::TextDelta("Let me calculate that.".to_string()),
                        AssistantEvent::ToolUse {
                            id: "tool-1".to_string(),
                            name: "add".to_string(),
                            input: "2,2".to_string(),
                        },
                        AssistantEvent::Usage(TokenUsage {
                            input_tokens: 20,
                            output_tokens: 6,
                            cache_creation_input_tokens: 1,
                            cache_read_input_tokens: 2,
                        }),
                        AssistantEvent::MessageStop,
                    ])
                }
                2 => {
                    let last_message = request
                        .messages
                        .last()
                        .expect("tool result should be present");
                    assert_eq!(last_message.role, MessageRole::Tool);
                    Ok(vec![
                        AssistantEvent::TextDelta("The answer is 4.".to_string()),
                        AssistantEvent::Usage(TokenUsage {
                            input_tokens: 24,
                            output_tokens: 4,
                            cache_creation_input_tokens: 1,
                            cache_read_input_tokens: 3,
                        }),
                        AssistantEvent::MessageStop,
                    ])
                }
                _ => Err(RuntimeError::new("unexpected extra API call")),
            }
        }
    }

    struct PromptAllowOnce;

    impl PermissionPrompter for PromptAllowOnce {
        fn decide(&mut self, request: &PermissionRequest) -> PermissionPromptDecision {
            assert_eq!(request.tool_name, "add");
            PermissionPromptDecision::Allow
        }
    }

    #[test]
    fn runs_user_to_tool_to_result_loop_end_to_end_and_tracks_usage() {
        let api_client = ScriptedApiClient { call_count: 0 };
        let tool_executor = StaticToolExecutor::new().register("add", |input| {
            let total = input
                .split(',')
                .map(|part| part.parse::<i32>().expect("input must be valid integer"))
                .sum::<i32>();
            Ok(total.to_string())
        });
        let permission_policy = PermissionPolicy::new(PermissionMode::WorkspaceWrite);
        let system_prompt = SystemPromptBuilder::new()
            .with_project_context(ProjectContext {
                cwd: PathBuf::from("/tmp/project"),
                current_date: "2026-03-31".to_string(),
                git_status: None,
                git_diff: None,
                instruction_files: Vec::new(),
            })
            .with_os("linux", "6.8")
            .build();
        let mut runtime = ConversationRuntime::new(
            Session::new(),
            api_client,
            tool_executor,
            permission_policy,
            system_prompt,
            (),
        );

        let summary = runtime
            .run_turn("what is 2 + 2?", Some(&mut PromptAllowOnce))
            .expect("conversation loop should succeed");

        assert_eq!(summary.iterations, 2);
        assert_eq!(summary.assistant_messages.len(), 2);
        assert_eq!(summary.tool_results.len(), 1);
        assert_eq!(runtime.session().messages.len(), 4);
        assert_eq!(summary.usage.output_tokens, 10);
        assert!(matches!(
            runtime.session().messages[1].blocks[1],
            ContentBlock::ToolUse { .. }
        ));
        assert!(matches!(
            runtime.session().messages[2].blocks[0],
            ContentBlock::ToolResult {
                is_error: false,
                ..
            }
        ));
    }

    #[test]
    fn records_denied_tool_results_when_prompt_rejects() {
        struct RejectPrompter;
        impl PermissionPrompter for RejectPrompter {
            fn decide(&mut self, _request: &PermissionRequest) -> PermissionPromptDecision {
                PermissionPromptDecision::Deny {
                    reason: "not now".to_string(),
                }
            }
        }

        struct SingleCallApiClient;
        impl ApiClient for SingleCallApiClient {
            fn active_model(&self) -> &str {
                "test-model"
            }

            fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
                if request
                    .messages
                    .iter()
                    .any(|message| message.role == MessageRole::Tool)
                {
                    return Ok(vec![
                        AssistantEvent::TextDelta("I could not use the tool.".to_string()),
                        AssistantEvent::MessageStop,
                    ]);
                }
                Ok(vec![
                    AssistantEvent::ToolUse {
                        id: "tool-1".to_string(),
                        name: "blocked".to_string(),
                        input: "secret".to_string(),
                    },
                    AssistantEvent::MessageStop,
                ])
            }
        }

        let mut runtime = ConversationRuntime::new(
            Session::new(),
            SingleCallApiClient,
            StaticToolExecutor::new(),
            PermissionPolicy::new(PermissionMode::WorkspaceWrite),
            SystemBlock::from_plain("system"),
            (),
        );

        let summary = runtime
            .run_turn("use the tool", Some(&mut RejectPrompter))
            .expect("conversation should continue after denied tool");

        assert_eq!(summary.tool_results.len(), 1);
        assert!(matches!(
            &summary.tool_results[0].blocks[0],
            ContentBlock::ToolResult { is_error: true, output, .. } if output == "not now"
        ));
    }

    #[test]
    #[cfg(unix)]
    fn denies_tool_use_when_pre_tool_hook_blocks() {
        struct SingleCallApiClient;
        impl ApiClient for SingleCallApiClient {
            fn active_model(&self) -> &str {
                "test-model"
            }

            fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
                if request
                    .messages
                    .iter()
                    .any(|message| message.role == MessageRole::Tool)
                {
                    return Ok(vec![
                        AssistantEvent::TextDelta("blocked".to_string()),
                        AssistantEvent::MessageStop,
                    ]);
                }
                Ok(vec![
                    AssistantEvent::ToolUse {
                        id: "tool-1".to_string(),
                        name: "blocked".to_string(),
                        input: r#"{"path":"secret.txt"}"#.to_string(),
                    },
                    AssistantEvent::MessageStop,
                ])
            }
        }

        let observer = HookDispatcher::from_hook_config(&RuntimeHookConfig::new(
            vec!["printf 'blocked by hook'; exit 2".to_string()],
            Vec::new(),
        ));
        let mut runtime = ConversationRuntime::new(
            Session::new(),
            SingleCallApiClient,
            StaticToolExecutor::new().register("blocked", |_input| {
                panic!("tool should not execute when hook denies")
            }),
            PermissionPolicy::new(PermissionMode::DangerFullAccess)
                .with_tool_requirement("blocked", PermissionMode::WorkspaceWrite),
            SystemBlock::from_plain("system"),
            observer,
        );

        let summary = runtime
            .run_turn("use the tool", None)
            .expect("conversation should continue after hook denial");

        assert_eq!(summary.tool_results.len(), 1);
        let ContentBlock::ToolResult {
            is_error, output, ..
        } = &summary.tool_results[0].blocks[0]
        else {
            panic!("expected tool result block");
        };
        assert!(
            *is_error,
            "hook denial should produce an error result: {output}"
        );
        assert!(
            output.contains("denied tool") || output.contains("blocked by hook"),
            "unexpected hook denial output: {output:?}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn appends_post_tool_hook_feedback_to_tool_result() {
        struct TwoCallApiClient {
            calls: usize,
        }

        impl ApiClient for TwoCallApiClient {
            fn active_model(&self) -> &str {
                "test-model"
            }

            fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
                self.calls += 1;
                match self.calls {
                    1 => Ok(vec![
                        AssistantEvent::ToolUse {
                            id: "tool-1".to_string(),
                            name: "add".to_string(),
                            input: r#"{"lhs":2,"rhs":2}"#.to_string(),
                        },
                        AssistantEvent::MessageStop,
                    ]),
                    2 => {
                        assert!(request
                            .messages
                            .iter()
                            .any(|message| message.role == MessageRole::Tool));
                        Ok(vec![
                            AssistantEvent::TextDelta("done".to_string()),
                            AssistantEvent::MessageStop,
                        ])
                    }
                    _ => Err(RuntimeError::new("unexpected extra API call")),
                }
            }
        }

        let observer = HookDispatcher::from_hook_config(&RuntimeHookConfig::new(
            vec!["printf 'pre hook ran'".to_string()],
            vec!["printf 'post hook ran'".to_string()],
        ));
        let mut runtime = ConversationRuntime::new(
            Session::new(),
            TwoCallApiClient { calls: 0 },
            StaticToolExecutor::new().register("add", |_input| Ok("4".to_string())),
            PermissionPolicy::new(PermissionMode::DangerFullAccess)
                .with_tool_requirement("add", PermissionMode::WorkspaceWrite),
            SystemBlock::from_plain("system"),
            observer,
        );

        let summary = runtime
            .run_turn("use add", None)
            .expect("tool loop succeeds");

        assert_eq!(summary.tool_results.len(), 1);
        let ContentBlock::ToolResult {
            is_error, output, ..
        } = &summary.tool_results[0].blocks[0]
        else {
            panic!("expected tool result block");
        };
        assert!(
            !*is_error,
            "post hook should preserve non-error result: {output:?}"
        );
        assert!(
            output.contains('4'),
            "tool output missing value: {output:?}"
        );
        assert!(
            output.contains("pre hook ran"),
            "tool output missing pre hook feedback: {output:?}"
        );
        assert!(
            output.contains("post hook ran"),
            "tool output missing post hook feedback: {output:?}"
        );
    }

    #[test]
    fn reconstructs_usage_tracker_from_restored_session() {
        struct SimpleApi;
        impl ApiClient for SimpleApi {
            fn active_model(&self) -> &str {
                "test-model"
            }

            fn stream(
                &mut self,
                _request: ApiRequest,
            ) -> Result<Vec<AssistantEvent>, RuntimeError> {
                Ok(vec![
                    AssistantEvent::TextDelta("done".to_string()),
                    AssistantEvent::MessageStop,
                ])
            }
        }

        let mut session = Session::new();
        session
            .messages
            .push(crate::session::ConversationMessage::assistant_with_usage(
                vec![ContentBlock::Text {
                    text: "earlier".to_string(),
                }],
                Some(TokenUsage {
                    input_tokens: 11,
                    output_tokens: 7,
                    cache_creation_input_tokens: 2,
                    cache_read_input_tokens: 1,
                }),
            ));

        let runtime = ConversationRuntime::new(
            session,
            SimpleApi,
            StaticToolExecutor::new(),
            PermissionPolicy::new(PermissionMode::DangerFullAccess),
            SystemBlock::from_plain("system"),
            (),
        );

        assert_eq!(runtime.usage().turns(), 1);
        assert_eq!(runtime.usage().cumulative_usage().total_tokens(), 21);
    }

    #[test]
    fn persists_user_message_before_first_api_call_when_session_path_set() {
        struct CheckPersistApi {
            path: PathBuf,
        }

        impl ApiClient for CheckPersistApi {
            fn active_model(&self) -> &str {
                "test-model"
            }

            fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
                assert!(
                    self.path.exists(),
                    "session file should exist before API stream"
                );
                let loaded = Session::load_from_path(&self.path).expect("session should load");
                assert!(
                    loaded
                        .messages
                        .iter()
                        .any(|m| matches!(m.role, MessageRole::User)),
                    "user message should be on disk before the API call"
                );
                assert!(request.messages.iter().any(|m| m.role == MessageRole::User));
                Ok(vec![
                    AssistantEvent::TextDelta("ok".to_string()),
                    AssistantEvent::MessageStop,
                ])
            }
        }

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("aineer-persist-turn-{nanos}.json"));

        let mut runtime = ConversationRuntime::new(
            Session::new(),
            CheckPersistApi { path: path.clone() },
            StaticToolExecutor::new(),
            PermissionPolicy::new(PermissionMode::WorkspaceWrite),
            SystemBlock::from_plain("system"),
            (),
        )
        .with_session_path(path.clone());

        runtime.run_turn("hi", None).expect("turn should succeed");
        fs::remove_file(&path).ok();
    }

    #[test]
    fn compacts_session_after_turns() {
        struct SimpleApi;
        impl ApiClient for SimpleApi {
            fn active_model(&self) -> &str {
                "test-model"
            }

            fn stream(
                &mut self,
                _request: ApiRequest,
            ) -> Result<Vec<AssistantEvent>, RuntimeError> {
                Ok(vec![
                    AssistantEvent::TextDelta("done".to_string()),
                    AssistantEvent::MessageStop,
                ])
            }
        }

        let mut runtime = ConversationRuntime::new(
            Session::new(),
            SimpleApi,
            StaticToolExecutor::new(),
            PermissionPolicy::new(PermissionMode::DangerFullAccess),
            SystemBlock::from_plain("system"),
            (),
        );
        runtime.run_turn("a", None).expect("turn a");
        runtime.run_turn("b", None).expect("turn b");
        runtime.run_turn("c", None).expect("turn c");

        let result = runtime.compact(CompactionConfig {
            preserve_recent_messages: 2,
            max_estimated_tokens: 1,
        });
        assert!(result.summary.contains("Conversation summary"));
        assert_eq!(
            result.compacted_session.messages[0].role,
            MessageRole::System
        );
    }

    /// Autocompact with model compaction **disabled** uses only local [`crate::compact::compact_session`]
    /// after a context-overflow API error — one extra main `stream` retry, no summarization call.
    #[test]
    fn autocompact_with_model_disabled_skips_summary_api() {
        struct AutocompactClient {
            calls: usize,
        }
        impl ApiClient for AutocompactClient {
            fn active_model(&self) -> &str {
                "test-model"
            }
            fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
                self.calls += 1;
                match self.calls {
                    1 => Err(RuntimeError::api(413, None, "context too long")),
                    2 => {
                        assert!(
                            request.max_output_tokens.is_none(),
                            "main turn should not cap output tokens"
                        );
                        Ok(vec![
                            AssistantEvent::TextDelta("ok".to_string()),
                            AssistantEvent::MessageStop,
                        ])
                    }
                    _ => panic!("unexpected stream call {}", self.calls),
                }
            }
        }

        // Default autocompact uses [`CompactionConfig::default()`] (`max_estimated_tokens` 10_000);
        // nine large user messages exceed that budget so [`should_compact`] is true.
        let mut session = Session::new();
        for _ in 0..9 {
            session
                .messages
                .push(ConversationMessage::user_text("x".repeat(5000)));
        }

        let mut runtime = ConversationRuntime::new(
            session,
            AutocompactClient { calls: 0 },
            StaticToolExecutor::new(),
            PermissionPolicy::new(PermissionMode::DangerFullAccess),
            SystemBlock::from_plain("system"),
            (),
        )
        .with_model_compaction(ModelCompactionConfig {
            enabled: false,
            ..ModelCompactionConfig::default()
        });

        runtime
            .run_turn("hi", None)
            .expect("turn after autocompact");
        assert!(
            runtime.session().messages.len() < 11,
            "session should have been compacted"
        );
    }

    /// With model compaction enabled, autocompact performs a summarization `stream` (with capped
    /// `max_output_tokens`) before retrying the main request when the model returns an empty summary,
    /// [`crate::compact::compact_session`] is used as fallback.
    #[test]
    fn autocompact_with_model_enabled_empty_summary_falls_back_to_heuristic() {
        struct Client {
            calls: usize,
        }
        impl ApiClient for Client {
            fn active_model(&self) -> &str {
                "test-model"
            }
            fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
                self.calls += 1;
                match self.calls {
                    1 => Err(RuntimeError::api(413, None, "context too long")),
                    2 => {
                        assert!(
                            request.max_output_tokens.is_some(),
                            "compaction summary should cap output tokens"
                        );
                        Ok(vec![AssistantEvent::MessageStop])
                    }
                    3 => {
                        assert!(request.max_output_tokens.is_none());
                        Ok(vec![
                            AssistantEvent::TextDelta("ok".to_string()),
                            AssistantEvent::MessageStop,
                        ])
                    }
                    _ => panic!("unexpected stream call {}", self.calls),
                }
            }
        }

        let mut session = Session::new();
        for _ in 0..9 {
            session
                .messages
                .push(ConversationMessage::user_text("x".repeat(5000)));
        }

        let mut runtime = ConversationRuntime::new(
            session,
            Client { calls: 0 },
            StaticToolExecutor::new(),
            PermissionPolicy::new(PermissionMode::DangerFullAccess),
            SystemBlock::from_plain("system"),
            (),
        )
        .with_model_compaction(ModelCompactionConfig {
            enabled: true,
            ..ModelCompactionConfig::default()
        });

        runtime
            .run_turn("hi", None)
            .expect("turn after autocompact");
    }
}
