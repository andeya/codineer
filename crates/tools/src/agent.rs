use std::collections::{BTreeMap, BTreeSet};

use crate::canonical_tool_token;
use crate::execute_tool;
use crate::registry::ToolSpec;
use crate::specs::mvp_tool_specs;
use crate::types::{AgentInput, AgentJob, AgentOutput, AgentResult, AgentRunStatus};
use aineer_api::{
    max_tokens_for_model, ContentBlockDelta, InputContentBlock, InputMessage, MessageRequest,
    MessageResponse, OutputContentBlock, ProviderClient, StreamEvent as ApiStreamEvent,
    SystemBlock, ToolChoice, ToolDefinition, ToolResultContentBlock,
};
use aineer_engine::{
    load_system_prompt, ApiClient, ApiRequest, AssistantEvent, ContentBlock, ConversationMessage,
    ConversationRuntime, MessageRole, PermissionMode, PermissionPolicy, RuntimeError, Session,
    TokenUsage, ToolError, ToolExecutor,
};

fn default_agent_model() -> String {
    aineer_api::auto_detect_default_model()
        .unwrap_or("claude-sonnet-4-6")
        .to_string()
}
const DEFAULT_AGENT_SYSTEM_DATE: &str = "2026-03-31";
const DEFAULT_AGENT_MAX_ITERATIONS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubagentKind {
    Explore,
    Plan,
    Verification,
    AineerGuide,
    StatuslineSetup,
    General,
}

impl SubagentKind {
    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "Explore" => Self::Explore,
            "Plan" => Self::Plan,
            "Verification" => Self::Verification,
            "aineer-guide" => Self::AineerGuide,
            "statusline-setup" => Self::StatuslineSetup,
            _ => Self::General,
        }
    }

    pub(crate) fn allowed_tools(self) -> BTreeSet<String> {
        let tools: &[&str] = match self {
            Self::Explore => &[
                "read_file",
                "glob_search",
                "grep_search",
                "WebFetch",
                "WebSearch",
                "ToolSearch",
                "Skill",
                "StructuredOutput",
            ],
            Self::Plan => &[
                "read_file",
                "glob_search",
                "grep_search",
                "WebFetch",
                "WebSearch",
                "ToolSearch",
                "Skill",
                "TodoWrite",
                "StructuredOutput",
                "SendUserMessage",
            ],
            Self::Verification => &[
                "bash",
                "read_file",
                "glob_search",
                "grep_search",
                "WebFetch",
                "WebSearch",
                "ToolSearch",
                "TodoWrite",
                "StructuredOutput",
                "SendUserMessage",
                "PowerShell",
            ],
            Self::AineerGuide => &[
                "read_file",
                "glob_search",
                "grep_search",
                "WebFetch",
                "WebSearch",
                "ToolSearch",
                "Skill",
                "StructuredOutput",
                "SendUserMessage",
            ],
            Self::StatuslineSetup => &[
                "bash",
                "read_file",
                "write_file",
                "edit_file",
                "glob_search",
                "grep_search",
                "ToolSearch",
            ],
            Self::General => &[
                "bash",
                "read_file",
                "write_file",
                "edit_file",
                "glob_search",
                "grep_search",
                "WebFetch",
                "WebSearch",
                "TodoWrite",
                "Skill",
                "ToolSearch",
                "NotebookEdit",
                "Sleep",
                "SendUserMessage",
                "Config",
                "StructuredOutput",
                "REPL",
                "PowerShell",
            ],
        };
        tools.iter().map(|s| (*s).to_string()).collect()
    }
}

pub(crate) fn execute_agent(input: AgentInput) -> Result<AgentOutput, String> {
    execute_agent_with_spawn(input, spawn_agent_job)
}

pub(crate) fn execute_agent_with_spawn<F>(
    input: AgentInput,
    spawn_fn: F,
) -> Result<AgentOutput, String>
where
    F: FnOnce(AgentJob) -> Result<(), String>,
{
    if input.description.trim().is_empty() {
        return Err(String::from("description must not be empty"));
    }
    if input.prompt.trim().is_empty() {
        return Err(String::from("prompt must not be empty"));
    }

    let agent_id = make_agent_id();
    let output_dir = agent_store_dir()?;
    std::fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;
    let output_file = output_dir.join(format!("{agent_id}.md"));
    let manifest_file = output_dir.join(format!("{agent_id}.json"));
    let normalized_subagent_type = normalize_subagent_type(input.subagent_type.as_deref());
    let model = resolve_agent_model(input.model.as_deref());
    let agent_name = input
        .name
        .as_deref()
        .map(slugify_agent_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| slugify_agent_name(&input.description));
    let created_at = iso8601_now();
    let system_prompt = build_agent_system_prompt(&normalized_subagent_type)?;
    let allowed_tools = allowed_tools_for_subagent(&normalized_subagent_type);

    let output_contents = format!(
        "# Agent Task

- id: {}
- name: {}
- description: {}
- subagent_type: {}
- created_at: {}

## Prompt

{}
",
        agent_id, agent_name, input.description, normalized_subagent_type, created_at, input.prompt
    );
    std::fs::write(&output_file, output_contents).map_err(|error| error.to_string())?;

    let manifest = AgentOutput {
        agent_id,
        name: agent_name,
        description: input.description,
        subagent_type: Some(normalized_subagent_type),
        model: Some(model),
        status: AgentRunStatus::Running,
        output_file: output_file.display().to_string(),
        manifest_file: manifest_file.display().to_string(),
        created_at: created_at.clone(),
        started_at: Some(created_at),
        completed_at: None,
        error: None,
        agent_result: None,
    };
    write_agent_manifest(&manifest)?;

    let manifest_for_spawn = manifest.clone();
    let job = AgentJob {
        manifest: manifest_for_spawn,
        prompt: input.prompt,
        system_prompt,
        allowed_tools,
    };
    if let Err(error) = spawn_fn(job) {
        let error = format!("failed to spawn sub-agent: {error}");
        persist_agent_terminal_state(
            &manifest,
            AgentRunStatus::Failed,
            None,
            Some(error.clone()),
            None,
        )?;
        return Err(error);
    }

    Ok(manifest)
}

fn spawn_agent_job(job: AgentJob) -> Result<(), String> {
    let thread_name = format!("aineer-agent-{}", job.manifest.agent_id);
    std::thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_agent_job(&job)));
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    let _ = persist_agent_terminal_state(
                        &job.manifest,
                        AgentRunStatus::Failed,
                        None,
                        Some(error),
                        None,
                    );
                }
                Err(_) => {
                    let _ = persist_agent_terminal_state(
                        &job.manifest,
                        AgentRunStatus::Failed,
                        None,
                        Some(String::from("sub-agent thread panicked")),
                        None,
                    );
                }
            }
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn run_agent_job(job: &AgentJob) -> Result<(), String> {
    let mut runtime = build_agent_runtime(job)?.with_max_iterations(DEFAULT_AGENT_MAX_ITERATIONS);
    let summary = runtime
        .run_turn(job.prompt.clone(), None)
        .map_err(|error| error.to_string())?;
    let agent_result = build_agent_result(&summary, true);
    let final_text = agent_result.summary.clone();
    persist_agent_terminal_state(
        &job.manifest,
        AgentRunStatus::Completed,
        Some(final_text.as_str()),
        None,
        Some(agent_result),
    )
}

fn build_agent_runtime(
    job: &AgentJob,
) -> Result<ConversationRuntime<ProviderRuntimeClient, SubagentToolExecutor>, String> {
    let model = job
        .manifest
        .model
        .clone()
        .unwrap_or_else(default_agent_model);
    let allowed_tools = job.allowed_tools.clone();
    let api_client = ProviderRuntimeClient::new(&model, allowed_tools.clone())?;
    let tool_executor = SubagentToolExecutor::new(allowed_tools);
    Ok(ConversationRuntime::new(
        Session::new(),
        api_client,
        tool_executor,
        agent_permission_policy(),
        job.system_prompt.clone(),
        (),
    ))
}

fn build_agent_system_prompt(subagent_type: &str) -> Result<Vec<SystemBlock>, String> {
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut prompt = load_system_prompt(
        cwd,
        DEFAULT_AGENT_SYSTEM_DATE.to_string(),
        std::env::consts::OS,
        "unknown",
    )
    .map_err(|error| error.to_string())?;
    prompt.push(SystemBlock::text(format!(
        "You are a background sub-agent of type `{subagent_type}`. Work only on the delegated task, use only the tools available to you, do not ask the user questions, and finish with a concise result."
    )));
    Ok(prompt)
}

fn resolve_agent_model(model: Option<&str>) -> String {
    model
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(str::to_string)
        .unwrap_or_else(default_agent_model)
}

pub(crate) fn allowed_tools_for_subagent(subagent_type: &str) -> BTreeSet<String> {
    SubagentKind::from_str(subagent_type).allowed_tools()
}

pub(crate) fn agent_permission_policy() -> PermissionPolicy {
    mvp_tool_specs().into_iter().fold(
        PermissionPolicy::new(PermissionMode::DangerFullAccess),
        |policy, spec| policy.with_tool_requirement(spec.name, spec.required_permission),
    )
}

fn write_agent_manifest(manifest: &AgentOutput) -> Result<(), String> {
    std::fs::write(
        &manifest.manifest_file,
        serde_json::to_string_pretty(manifest).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn persist_agent_terminal_state(
    manifest: &AgentOutput,
    status: AgentRunStatus,
    result: Option<&str>,
    error: Option<String>,
    agent_result: Option<AgentResult>,
) -> Result<(), String> {
    let status_str = status.to_string();
    append_agent_output(
        &manifest.output_file,
        &format_agent_terminal_output(&status_str, result, error.as_deref(), agent_result.as_ref()),
    )?;
    let mut next_manifest = manifest.clone();
    next_manifest.status = status;
    next_manifest.completed_at = Some(iso8601_now());
    next_manifest.error = error;
    next_manifest.agent_result = agent_result;
    write_agent_manifest(&next_manifest)
}

fn append_agent_output(path: &str, suffix: &str) -> Result<(), String> {
    use std::io::Write as _;

    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(suffix.as_bytes())
        .map_err(|error| error.to_string())
}

fn format_agent_terminal_output(
    status: &str,
    result: Option<&str>,
    error: Option<&str>,
    agent_result: Option<&AgentResult>,
) -> String {
    let mut sections = vec![format!("\n## Result\n\n- status: {status}\n")];
    if let Some(result) = result.filter(|value| !value.trim().is_empty()) {
        sections.push(format!("\n### Final response\n\n{}\n", result.trim()));
    }
    if let Some(error) = error.filter(|value| !value.trim().is_empty()) {
        sections.push(format!("\n### Error\n\n{}\n", error.trim()));
    }
    if let Some(structured) = agent_result {
        if let Ok(json) = serde_json::to_string_pretty(structured) {
            sections.push(format!(
                "\n### Structured agent result\n\n```json\n{json}\n```\n"
            ));
        }
    }
    sections.join("")
}

/// Builds a structured summary after a sub-agent turn, including paths touched by file tools.
pub(crate) fn build_agent_result(
    summary: &aineer_engine::TurnSummary,
    success: bool,
) -> AgentResult {
    AgentResult {
        summary: final_assistant_text(summary),
        files_modified: collect_files_modified_from_turn(summary),
        success,
    }
}

fn collect_files_modified_from_turn(summary: &aineer_engine::TurnSummary) -> Vec<String> {
    use aineer_engine::ContentBlock;

    let mut paths = BTreeSet::new();
    for message in &summary.assistant_messages {
        for block in &message.blocks {
            let ContentBlock::ToolUse { name, input, .. } = block else {
                continue;
            };
            if let Some(path) = path_from_tool_use_input(name, input) {
                paths.insert(path);
            }
        }
    }
    paths.into_iter().collect()
}

fn path_from_tool_use_input(tool_name: &str, input_json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(input_json).ok()?;
    match tool_name {
        "write_file" | "edit_file" | "MultiEdit" => value
            .get("path")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        "NotebookEdit" => value
            .get("notebook_path")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        _ => None,
    }
}

struct ProviderRuntimeClient {
    runtime: tokio::runtime::Runtime,
    client: ProviderClient,
    model: String,
    allowed_tools: BTreeSet<String>,
}

impl ProviderRuntimeClient {
    fn new(model: &str, allowed_tools: BTreeSet<String>) -> Result<Self, String> {
        let model = model.trim().to_string();
        let client = ProviderClient::from_model(&model).map_err(|error| error.to_string())?;
        Ok(Self {
            runtime: tokio::runtime::Runtime::new().map_err(|error| error.to_string())?,
            client,
            model,
            allowed_tools,
        })
    }
}

impl ApiClient for ProviderRuntimeClient {
    fn active_model(&self) -> &str {
        &self.model
    }

    #[allow(clippy::too_many_lines)]
    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
        let tools = tool_specs_for_allowed_tools(Some(&self.allowed_tools))
            .into_iter()
            .map(|spec| ToolDefinition {
                name: spec.name.to_string(),
                description: Some(spec.description.to_string()),
                input_schema: spec.input_schema,
                cache_control: None,
            })
            .collect::<Vec<_>>();
        let message_request = MessageRequest {
            model: self.model.clone(),
            max_tokens: max_tokens_for_model(&self.model),
            messages: convert_messages(&request.messages),
            system: (!request.system_prompt.is_empty()).then(|| request.system_prompt.clone()),
            tools: (!tools.is_empty()).then_some(tools),
            tool_choice: (!self.allowed_tools.is_empty()).then_some(ToolChoice::Auto),
            stream: true,
            thinking: None,
            gemini_cached_content: None,
        };

        self.runtime.block_on(async {
            let mut stream = self
                .client
                .stream_message(&message_request)
                .await
                .map_err(aineer_api::ApiError::into_runtime_error)?;
            let mut events = Vec::new();
            let mut pending_tools: BTreeMap<u32, (String, String, String)> = BTreeMap::new();
            let mut saw_stop = false;

            while let Some(event) = stream
                .next_event()
                .await
                .map_err(aineer_api::ApiError::into_runtime_error)?
            {
                match event {
                    ApiStreamEvent::MessageStart(start) => {
                        for block in start.message.content {
                            push_output_block(block, 0, &mut events, &mut pending_tools, true);
                        }
                    }
                    ApiStreamEvent::ContentBlockStart(start) => {
                        push_output_block(
                            start.content_block,
                            start.index,
                            &mut events,
                            &mut pending_tools,
                            true,
                        );
                    }
                    ApiStreamEvent::ContentBlockDelta(delta) => match delta.delta {
                        ContentBlockDelta::TextDelta { text } if !text.is_empty() => {
                            events.push(AssistantEvent::TextDelta(text));
                        }
                        ContentBlockDelta::InputJsonDelta { partial_json } => {
                            if let Some((_, _, input)) = pending_tools.get_mut(&delta.index) {
                                input.push_str(&partial_json);
                            }
                        }
                        ContentBlockDelta::ThinkingDelta { thinking } if !thinking.is_empty() => {
                            events.push(AssistantEvent::ThinkingDelta(thinking));
                        }
                        ContentBlockDelta::SignatureDelta { .. } => {}
                        _ => {}
                    },
                    ApiStreamEvent::ContentBlockStop(stop) => {
                        if let Some((id, name, input)) = pending_tools.remove(&stop.index) {
                            events.push(AssistantEvent::ToolUse { id, name, input });
                        }
                    }
                    ApiStreamEvent::MessageDelta(delta) => {
                        events.push(AssistantEvent::Usage(TokenUsage {
                            input_tokens: delta.usage.input_tokens,
                            output_tokens: delta.usage.output_tokens,
                            cache_creation_input_tokens: 0,
                            cache_read_input_tokens: 0,
                        }));
                    }
                    ApiStreamEvent::MessageStop(_) => {
                        saw_stop = true;
                        events.push(AssistantEvent::MessageStop);
                    }
                    _ => {}
                }
            }

            if !saw_stop
                && events.iter().any(|event| {
                    matches!(event, AssistantEvent::TextDelta(text) if !text.is_empty())
                        || matches!(event, AssistantEvent::ThinkingDelta(text) if !text.is_empty())
                        || matches!(event, AssistantEvent::ToolUse { .. })
                })
            {
                events.push(AssistantEvent::MessageStop);
            }

            if events
                .iter()
                .any(|event| matches!(event, AssistantEvent::MessageStop))
            {
                return Ok(events);
            }

            let response = self
                .client
                .send_message(&MessageRequest {
                    stream: false,
                    thinking: None,
                    ..message_request.clone()
                })
                .await
                .map_err(aineer_api::ApiError::into_runtime_error)?;
            Ok(response_to_events(response))
        })
    }
}

pub(crate) struct SubagentToolExecutor {
    allowed_tools: BTreeSet<String>,
}

impl SubagentToolExecutor {
    pub(crate) fn new(allowed_tools: BTreeSet<String>) -> Self {
        Self { allowed_tools }
    }
}

impl ToolExecutor for SubagentToolExecutor {
    fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, ToolError> {
        if !self.allowed_tools.contains(tool_name) {
            return Err(ToolError::new(format!(
                "tool `{tool_name}` is not enabled for this sub-agent"
            )));
        }
        let value = serde_json::from_str(input)
            .map_err(|error| ToolError::new(format!("invalid tool input JSON: {error}")))?;
        execute_tool(tool_name, value)
            .map(|o| o.content)
            .map_err(|e| ToolError::new(e.to_string()))
    }
}

fn tool_specs_for_allowed_tools(allowed_tools: Option<&BTreeSet<String>>) -> Vec<ToolSpec> {
    mvp_tool_specs()
        .into_iter()
        .filter(|spec| allowed_tools.is_none_or(|allowed| allowed.contains(spec.name)))
        .collect()
}

fn convert_messages(messages: &[ConversationMessage]) -> Vec<InputMessage> {
    messages
        .iter()
        .filter_map(|message| {
            let role = match message.role {
                MessageRole::System | MessageRole::User | MessageRole::Tool => "user",
                MessageRole::Assistant => "assistant",
                _ => "user",
            };
            let content = message
                .blocks
                .iter()
                .map(|block| match block {
                    ContentBlock::Text { text } => InputContentBlock::Text {
                        text: text.clone(),
                        cache_control: None,
                    },
                    ContentBlock::Image { media_type, data } => InputContentBlock::Image {
                        source: aineer_api::ImageSource {
                            source_type: "base64".to_string(),
                            media_type: media_type.clone(),
                            data: data.clone(),
                        },
                    },
                    ContentBlock::ToolUse { id, name, input } => InputContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: serde_json::from_str(input)
                            .unwrap_or_else(|_| serde_json::json!({ "raw": input })),
                    },
                    ContentBlock::ToolResult {
                        tool_use_id,
                        output,
                        is_error,
                        ..
                    } => InputContentBlock::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: vec![ToolResultContentBlock::Text {
                            text: output.clone(),
                        }],
                        is_error: *is_error,
                        cache_control: None,
                    },
                    _ => InputContentBlock::Text {
                        text: String::new(),
                        cache_control: None,
                    },
                })
                .collect::<Vec<_>>();
            (!content.is_empty()).then(|| InputMessage {
                role: role.to_string(),
                content,
            })
        })
        .collect()
}

pub(crate) fn push_output_block(
    block: OutputContentBlock,
    block_index: u32,
    events: &mut Vec<AssistantEvent>,
    pending_tools: &mut BTreeMap<u32, (String, String, String)>,
    streaming_tool_input: bool,
) {
    match block {
        OutputContentBlock::Text { text } if !text.is_empty() => {
            events.push(AssistantEvent::TextDelta(text));
        }
        OutputContentBlock::ToolUse { id, name, input } => {
            let initial_input = if streaming_tool_input
                && input.is_object()
                && input.as_object().is_some_and(serde_json::Map::is_empty)
            {
                String::new()
            } else {
                input.to_string()
            };
            pending_tools.insert(block_index, (id, name, initial_input));
        }
        OutputContentBlock::Thinking { .. } | OutputContentBlock::RedactedThinking { .. } => {}
        _ => {}
    }
}

fn response_to_events(response: MessageResponse) -> Vec<AssistantEvent> {
    let mut events = Vec::new();
    let mut pending_tools = BTreeMap::new();

    for (index, block) in response.content.into_iter().enumerate() {
        let index = u32::try_from(index).expect("response block index overflow");
        push_output_block(block, index, &mut events, &mut pending_tools, false);
        if let Some((id, name, input)) = pending_tools.remove(&index) {
            events.push(AssistantEvent::ToolUse { id, name, input });
        }
    }

    events.push(AssistantEvent::Usage(TokenUsage {
        input_tokens: response.usage.input_tokens,
        output_tokens: response.usage.output_tokens,
        cache_creation_input_tokens: response.usage.cache_creation_input_tokens,
        cache_read_input_tokens: response.usage.cache_read_input_tokens,
    }));
    events.push(AssistantEvent::MessageStop);
    events
}

pub(crate) fn final_assistant_text(summary: &aineer_engine::TurnSummary) -> String {
    summary
        .assistant_messages
        .last()
        .map(|message| {
            message
                .blocks
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}
fn agent_store_dir() -> Result<std::path::PathBuf, String> {
    if let Ok(path) = std::env::var("AINEER_AGENT_STORE") {
        return Ok(std::path::PathBuf::from(path));
    }
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    Ok(aineer_engine::aineer_runtime_dir(&cwd).join("agents"))
}

fn make_agent_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("agent-{nanos}")
}

fn slugify_agent_name(description: &str) -> String {
    let mut out = description
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    out.trim_matches('-').chars().take(32).collect()
}

fn normalize_subagent_type(subagent_type: Option<&str>) -> String {
    let trimmed = subagent_type.map(str::trim).unwrap_or_default();
    if trimmed.is_empty() {
        return String::from("general-purpose");
    }

    match canonical_tool_token(trimmed).as_str() {
        "general" | "generalpurpose" | "generalpurposeagent" => String::from("general-purpose"),
        "explore" | "explorer" | "exploreagent" => String::from("Explore"),
        "plan" | "planagent" => String::from("Plan"),
        "verification" | "verificationagent" | "verify" | "verifier" => {
            String::from("Verification")
        }
        "aineerguide" | "aineerguideagent" | "guide" => String::from("aineer-guide"),
        "statusline" | "statuslinesetup" => String::from("statusline-setup"),
        _ => trimmed.to_string(),
    }
}

pub(crate) fn iso8601_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = i64::try_from(secs / 86400).unwrap_or(0);
    let day_secs = (secs % 86400) as u32;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        day_secs / 3600,
        (day_secs % 3600) / 60,
        day_secs % 60,
    )
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = u32::try_from(z - era * 146_097).unwrap_or(0);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = i32::try_from(i64::from(yoe) + era * 400).unwrap_or(0);
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::build_agent_result;
    use aineer_engine::{ContentBlock, ConversationMessage, MessageRole, TokenUsage, TurnSummary};

    #[test]
    fn agent_result_includes_summary_and_file_paths() {
        let summary = TurnSummary {
            assistant_messages: vec![
                ConversationMessage {
                    role: MessageRole::Assistant,
                    blocks: vec![ContentBlock::ToolUse {
                        id: "tu_1".into(),
                        name: "write_file".into(),
                        input: r#"{"path":"src/out.txt","content":"x"}"#.into(),
                    }],
                    usage: None,
                },
                ConversationMessage {
                    role: MessageRole::Assistant,
                    blocks: vec![ContentBlock::Text {
                        text: "Wrote the file.".into(),
                    }],
                    usage: None,
                },
            ],
            tool_results: vec![],
            iterations: 2,
            usage: TokenUsage::default(),
        };
        let result = build_agent_result(&summary, true);
        assert!(result.success);
        assert_eq!(result.summary, "Wrote the file.");
        assert_eq!(result.files_modified, vec!["src/out.txt"]);
    }

    #[test]
    fn agent_result_notebook_and_multi_edit_paths() {
        let summary = TurnSummary {
            assistant_messages: vec![ConversationMessage {
                role: MessageRole::Assistant,
                blocks: vec![
                    ContentBlock::ToolUse {
                        id: "tu_1".into(),
                        name: "MultiEdit".into(),
                        input: r#"{"path":"a.rs","edits":[]}"#.into(),
                    },
                    ContentBlock::ToolUse {
                        id: "tu_2".into(),
                        name: "NotebookEdit".into(),
                        input: r#"{"notebook_path":"n.ipynb","new_source":"x"}"#.into(),
                    },
                ],
                usage: None,
            }],
            tool_results: vec![],
            iterations: 1,
            usage: TokenUsage::default(),
        };
        let result = build_agent_result(&summary, true);
        assert_eq!(result.files_modified, vec!["a.rs", "n.ipynb"]);
    }

    #[test]
    fn agent_result_propagates_success_flag() {
        let summary = TurnSummary {
            assistant_messages: vec![ConversationMessage {
                role: MessageRole::Assistant,
                blocks: vec![ContentBlock::Text {
                    text: "Stopped early.".into(),
                }],
                usage: None,
            }],
            tool_results: vec![],
            iterations: 1,
            usage: TokenUsage::default(),
        };
        let ok = build_agent_result(&summary, true);
        let failed = build_agent_result(&summary, false);
        assert!(ok.success);
        assert!(!failed.success);
        assert_eq!(ok.summary, failed.summary);
        assert_eq!(ok.files_modified, failed.files_modified);
    }
}
