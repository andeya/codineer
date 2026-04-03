use std::io::Write;
use std::sync::Arc;

use api::{
    ContentBlockDelta, InputContentBlock, InputMessage, MessageRequest, MessageResponse,
    OutputContentBlock, ProviderClient, ProviderKind, StreamEvent as ApiStreamEvent, ToolChoice,
    ToolResultContentBlock,
};
use runtime::{
    ApiClient, ApiRequest, AssistantEvent, ConversationMessage, ConversationRuntime, MessageRole,
    PermissionMode, PermissionPolicy, RuntimeError, TokenUsage, ToolError, ToolExecutor,
};
use tools::GlobalToolRegistry;

use crate::auth::{no_credentials_error, provider_hint, resolve_cli_auth_source};
use crate::cli::{discover_mcp_tools, filter_tool_specs, AllowedToolSet, SharedMcpManager};
use crate::progress::InternalPromptProgressReporter;
use crate::render::{MarkdownStreamState, TerminalRenderer};
use crate::tool_display::format_tool_call_start;
use crate::{build_runtime_plugin_state, max_tokens_for_model};

pub(crate) struct RuntimeParams {
    pub(crate) session: runtime::Session,
    pub(crate) model: String,
    pub(crate) system_prompt: Vec<String>,
    pub(crate) enable_tools: bool,
    pub(crate) emit_output: bool,
    pub(crate) allowed_tools: Option<AllowedToolSet>,
    pub(crate) permission_mode: PermissionMode,
    pub(crate) progress_reporter: Option<InternalPromptProgressReporter>,
    pub(crate) mcp_manager: SharedMcpManager,
}

pub(crate) fn build_runtime(
    params: RuntimeParams,
) -> Result<ConversationRuntime<DefaultRuntimeClient, CliToolExecutor>, Box<dyn std::error::Error>>
{
    let RuntimeParams {
        session,
        model,
        system_prompt,
        enable_tools,
        emit_output,
        allowed_tools,
        permission_mode,
        progress_reporter,
        mcp_manager,
    } = params;
    let (feature_config, tool_registry) = build_runtime_plugin_state()?;
    Ok(ConversationRuntime::new_with_features(
        session,
        DefaultRuntimeClient::new(
            model,
            enable_tools,
            emit_output,
            allowed_tools.clone(),
            tool_registry.clone(),
            progress_reporter,
            Arc::clone(&mcp_manager),
        )?,
        CliToolExecutor::new(
            allowed_tools.clone(),
            emit_output,
            tool_registry.clone(),
            Arc::clone(&mcp_manager),
        ),
        permission_policy(permission_mode, &tool_registry),
        system_prompt,
        &feature_config,
    ))
}

pub(crate) struct CliPermissionPrompter {
    current_mode: PermissionMode,
}

impl CliPermissionPrompter {
    pub(crate) fn new(current_mode: PermissionMode) -> Self {
        Self { current_mode }
    }
}

impl runtime::PermissionPrompter for CliPermissionPrompter {
    fn decide(
        &mut self,
        request: &runtime::PermissionRequest,
    ) -> runtime::PermissionPromptDecision {
        println!();
        println!("Permission approval required");
        println!("  Tool             {}", request.tool_name);
        println!("  Current mode     {}", self.current_mode.as_str());
        println!("  Required mode    {}", request.required_mode.as_str());
        println!("  Input            {}", request.input);
        print!("Approve this tool call? [y/N]: ");
        let _ = std::io::stdout().flush();

        let mut response = String::new();
        match std::io::stdin().read_line(&mut response) {
            Ok(_) => {
                let normalized = response.trim().to_ascii_lowercase();
                if matches!(normalized.as_str(), "y" | "yes") {
                    runtime::PermissionPromptDecision::Allow
                } else {
                    runtime::PermissionPromptDecision::Deny {
                        reason: format!(
                            "tool '{}' denied by user approval prompt",
                            request.tool_name
                        ),
                    }
                }
            }
            Err(error) => runtime::PermissionPromptDecision::Deny {
                reason: format!("permission approval failed: {error}"),
            },
        }
    }
}

pub(crate) struct DefaultRuntimeClient {
    runtime: tokio::runtime::Runtime,
    client: ProviderClient,
    model: String,
    enable_tools: bool,
    emit_output: bool,
    allowed_tools: Option<AllowedToolSet>,
    tool_registry: GlobalToolRegistry,
    progress_reporter: Option<InternalPromptProgressReporter>,
    mcp_manager: SharedMcpManager,
}

impl DefaultRuntimeClient {
    pub(crate) fn new(
        model: String,
        enable_tools: bool,
        emit_output: bool,
        allowed_tools: Option<AllowedToolSet>,
        tool_registry: GlobalToolRegistry,
        progress_reporter: Option<InternalPromptProgressReporter>,
        mcp_manager: SharedMcpManager,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let model = if model == "auto" {
            api::auto_detect_default_model()
                .ok_or_else(no_credentials_error)?
                .to_string()
        } else {
            model
        };
        let resolved = api::resolve_model_alias(&model);
        let provider_kind = api::detect_provider_kind(&resolved);
        let auth = if provider_kind == ProviderKind::CodineerApi {
            Some(resolve_cli_auth_source().map_err(|err| provider_hint(&model, &err))?)
        } else {
            None
        };
        let client = ProviderClient::from_model_with_default_auth(&resolved, auth)
            .map_err(|err| provider_hint(&model, &err))?;
        Ok(Self {
            runtime: tokio::runtime::Runtime::new()?,
            client,
            model,
            enable_tools,
            emit_output,
            allowed_tools,
            tool_registry,
            progress_reporter,
            mcp_manager,
        })
    }

    fn build_message_request(&self, request: &ApiRequest) -> MessageRequest {
        MessageRequest {
            model: self.model.clone(),
            max_tokens: max_tokens_for_model(&self.model),
            messages: convert_messages(&request.messages),
            system: (!request.system_prompt.is_empty()).then(|| request.system_prompt.join("\n\n")),
            tools: self.enable_tools.then(|| {
                let mut specs = filter_tool_specs(&self.tool_registry, self.allowed_tools.as_ref());
                specs.extend(discover_mcp_tools(&self.runtime, &self.mcp_manager));
                specs
            }),
            tool_choice: self.enable_tools.then_some(ToolChoice::Auto),
            stream: true,
        }
    }
}

pub(crate) fn write_flush(out: &mut dyn Write, buf: &str) -> Result<(), RuntimeError> {
    write!(out, "{buf}")
        .and_then(|()| out.flush())
        .map_err(|error| RuntimeError::new(error.to_string()))
}

pub(crate) struct StreamState {
    renderer: TerminalRenderer,
    markdown_stream: MarkdownStreamState,
    events: Vec<AssistantEvent>,
    pending_tool: Option<(String, String, String)>,
    saw_stop: bool,
}

impl StreamState {
    fn new() -> Self {
        Self {
            renderer: TerminalRenderer::new(),
            markdown_stream: MarkdownStreamState::default(),
            events: Vec::new(),
            pending_tool: None,
            saw_stop: false,
        }
    }

    fn handle_event(
        &mut self,
        event: ApiStreamEvent,
        progress: Option<&InternalPromptProgressReporter>,
        out: &mut dyn Write,
    ) -> Result<(), RuntimeError> {
        match event {
            ApiStreamEvent::MessageStart(start) => {
                for block in start.message.content {
                    push_output_block(block, out, &mut self.events, &mut self.pending_tool, true)?;
                }
            }
            ApiStreamEvent::ContentBlockStart(start) => {
                push_output_block(
                    start.content_block,
                    out,
                    &mut self.events,
                    &mut self.pending_tool,
                    true,
                )?;
            }
            ApiStreamEvent::ContentBlockDelta(delta) => match delta.delta {
                ContentBlockDelta::TextDelta { text } => {
                    if !text.is_empty() {
                        if let Some(reporter) = progress {
                            reporter.mark_text_phase(&text);
                        }
                        if let Some(rendered) = self.markdown_stream.push(&self.renderer, &text) {
                            write_flush(out, &rendered)?;
                        }
                        self.events.push(AssistantEvent::TextDelta(text));
                    }
                }
                ContentBlockDelta::InputJsonDelta { partial_json } => {
                    if let Some((_, _, input)) = &mut self.pending_tool {
                        input.push_str(&partial_json);
                    }
                }
                ContentBlockDelta::ThinkingDelta { .. }
                | ContentBlockDelta::SignatureDelta { .. } => {}
            },
            ApiStreamEvent::ContentBlockStop(_) => {
                if let Some(rendered) = self.markdown_stream.flush(&self.renderer) {
                    write_flush(out, &rendered)?;
                }
                if let Some((id, name, input)) = self.pending_tool.take() {
                    if let Some(reporter) = progress {
                        reporter.mark_tool_phase(&name, &input);
                    }
                    let display = format!("\n{}", format_tool_call_start(&name, &input));
                    writeln!(out, "{display}")
                        .and_then(|()| out.flush())
                        .map_err(|error| RuntimeError::new(error.to_string()))?;
                    self.events
                        .push(AssistantEvent::ToolUse { id, name, input });
                }
            }
            ApiStreamEvent::MessageDelta(delta) => {
                self.events.push(AssistantEvent::Usage(TokenUsage {
                    input_tokens: delta.usage.input_tokens,
                    output_tokens: delta.usage.output_tokens,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                }));
            }
            ApiStreamEvent::MessageStop(_) => {
                self.saw_stop = true;
                if let Some(rendered) = self.markdown_stream.flush(&self.renderer) {
                    write_flush(out, &rendered)?;
                }
                self.events.push(AssistantEvent::MessageStop);
            }
        }
        Ok(())
    }

    fn ensure_stop_event(mut self) -> Vec<AssistantEvent> {
        if !self.saw_stop
            && self.events.iter().any(|event| {
                matches!(event, AssistantEvent::TextDelta(text) if !text.is_empty())
                    || matches!(event, AssistantEvent::ToolUse { .. })
            })
        {
            self.events.push(AssistantEvent::MessageStop);
        }
        self.events
    }
}

impl ApiClient for DefaultRuntimeClient {
    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
        if let Some(progress_reporter) = &self.progress_reporter {
            progress_reporter.mark_model_phase();
        }
        let message_request = self.build_message_request(&request);

        self.runtime.block_on(async {
            let mut stream = self
                .client
                .stream_message(&message_request)
                .await
                .map_err(|error| RuntimeError::new(error.to_string()))?;
            let mut stdout = std::io::stdout();
            let mut sink = std::io::sink();
            let out: &mut dyn Write = if self.emit_output {
                &mut stdout
            } else {
                &mut sink
            };
            let mut state = StreamState::new();
            while let Some(event) = stream
                .next_event()
                .await
                .map_err(|error| RuntimeError::new(error.to_string()))?
            {
                state.handle_event(event, self.progress_reporter.as_ref(), out)?;
            }

            let events = state.ensure_stop_event();
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
                    ..message_request.clone()
                })
                .await
                .map_err(|error| RuntimeError::new(error.to_string()))?;
            response_to_events(response, out)
        })
    }
}

pub(crate) fn push_output_block(
    block: OutputContentBlock,
    out: &mut (impl Write + ?Sized),
    events: &mut Vec<AssistantEvent>,
    pending_tool: &mut Option<(String, String, String)>,
    streaming_tool_input: bool,
) -> Result<(), RuntimeError> {
    match block {
        OutputContentBlock::Text { text } => {
            if !text.is_empty() {
                let rendered = TerminalRenderer::new().render_markdown(&text);
                write!(out, "{rendered}")
                    .and_then(|()| out.flush())
                    .map_err(|error| RuntimeError::new(error.to_string()))?;
                events.push(AssistantEvent::TextDelta(text));
            }
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
            *pending_tool = Some((id, name, initial_input));
        }
        OutputContentBlock::Thinking { .. } | OutputContentBlock::RedactedThinking { .. } => {}
    }
    Ok(())
}

pub(crate) fn response_to_events(
    response: MessageResponse,
    out: &mut (impl Write + ?Sized),
) -> Result<Vec<AssistantEvent>, RuntimeError> {
    let mut events = Vec::new();
    let mut pending_tool = None;

    for block in response.content {
        push_output_block(block, out, &mut events, &mut pending_tool, false)?;
        if let Some((id, name, input)) = pending_tool.take() {
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
    Ok(events)
}

pub(crate) struct CliToolExecutor {
    renderer: TerminalRenderer,
    emit_output: bool,
    allowed_tools: Option<AllowedToolSet>,
    tool_registry: GlobalToolRegistry,
    mcp_manager: SharedMcpManager,
}

impl CliToolExecutor {
    pub(crate) fn new(
        allowed_tools: Option<AllowedToolSet>,
        emit_output: bool,
        tool_registry: GlobalToolRegistry,
        mcp_manager: SharedMcpManager,
    ) -> Self {
        Self {
            renderer: TerminalRenderer::new(),
            emit_output,
            allowed_tools,
            tool_registry,
            mcp_manager,
        }
    }

    fn execute_mcp_tool(&mut self, tool_name: &str, input: &str) -> Result<String, ToolError> {
        let arguments: Option<serde_json::Value> = if input.trim().is_empty() {
            None
        } else {
            Some(
                serde_json::from_str(input)
                    .map_err(|e| ToolError::new(format!("invalid MCP tool input JSON: {e}")))?,
            )
        };
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| ToolError::new(format!("failed to create async runtime: {e}")))?;
        let mut guard = self
            .mcp_manager
            .lock()
            .map_err(|e| ToolError::new(format!("MCP manager lock poisoned: {e}")))?;
        let response = rt
            .block_on(guard.call_tool(tool_name, arguments))
            .map_err(|e| ToolError::new(format!("MCP tool call failed: {e}")))?;
        match response.result {
            Some(result) => {
                let text = result
                    .content
                    .iter()
                    .filter_map(|block| block.data.get("text").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(text)
            }
            None => {
                if let Some(error) = response.error {
                    Err(ToolError::new(format!(
                        "MCP error ({}): {}",
                        error.code, error.message
                    )))
                } else {
                    Ok(String::new())
                }
            }
        }
    }
}

impl ToolExecutor for CliToolExecutor {
    fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, ToolError> {
        if self
            .allowed_tools
            .as_ref()
            .is_some_and(|allowed| !allowed.contains(tool_name))
        {
            return Err(ToolError::new(format!(
                "tool `{tool_name}` is not enabled by the current --allowedTools setting"
            )));
        }

        let is_mcp_tool = tool_name.starts_with("mcp__");
        if is_mcp_tool {
            let result = self.execute_mcp_tool(tool_name, input);
            match &result {
                Ok(output) if self.emit_output => {
                    let markdown =
                        crate::tool_display::format_tool_result(tool_name, output, false);
                    let _ = self
                        .renderer
                        .stream_markdown(&markdown, &mut std::io::stdout());
                }
                Err(error) if self.emit_output => {
                    let markdown = crate::tool_display::format_tool_result(
                        tool_name,
                        &error.to_string(),
                        true,
                    );
                    let _ = self
                        .renderer
                        .stream_markdown(&markdown, &mut std::io::stdout());
                }
                _ => {}
            }
            return result;
        }

        let value = serde_json::from_str(input)
            .map_err(|error| ToolError::new(format!("invalid tool input JSON: {error}")))?;
        match self.tool_registry.execute(tool_name, &value) {
            Ok(output) => {
                if self.emit_output {
                    let markdown =
                        crate::tool_display::format_tool_result(tool_name, &output, false);
                    self.renderer
                        .stream_markdown(&markdown, &mut std::io::stdout())
                        .map_err(|error| ToolError::new(error.to_string()))?;
                }
                Ok(output)
            }
            Err(error) => {
                if self.emit_output {
                    let markdown = crate::tool_display::format_tool_result(tool_name, &error, true);
                    self.renderer
                        .stream_markdown(&markdown, &mut std::io::stdout())
                        .map_err(|stream_error| ToolError::new(stream_error.to_string()))?;
                }
                Err(ToolError::new(error))
            }
        }
    }
}

pub(crate) fn permission_policy(
    mode: PermissionMode,
    tool_registry: &GlobalToolRegistry,
) -> PermissionPolicy {
    tool_registry.permission_specs(None).into_iter().fold(
        PermissionPolicy::new(mode),
        |policy, (name, required_permission)| {
            policy.with_tool_requirement(name, required_permission)
        },
    )
}

pub(crate) fn convert_messages(messages: &[ConversationMessage]) -> Vec<InputMessage> {
    use runtime::ContentBlock;
    messages
        .iter()
        .filter_map(|message| {
            let role = match message.role {
                MessageRole::System | MessageRole::User | MessageRole::Tool => "user",
                MessageRole::Assistant => "assistant",
            };
            let content = message
                .blocks
                .iter()
                .map(|block| match block {
                    ContentBlock::Text { text } => InputContentBlock::Text { text: text.clone() },
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
