use std::collections::BTreeMap;
use std::io::Write;
use std::sync::Arc;

use api::{
    ContentBlockDelta, InputContentBlock, InputMessage, MessageRequest, MessageResponse,
    OpenAiCompatClient, OutputContentBlock, ProviderClient, ProviderKind,
    StreamEvent as ApiStreamEvent, ToolChoice, ToolResultContentBlock,
};
use runtime::{
    ApiClient, ApiRequest, AssistantEvent, ConversationMessage, ConversationRuntime,
    CustomProviderConfig, MessageRole, PermissionMode, PermissionPolicy, RuntimeError, TokenUsage,
    ToolError, ToolExecutor,
};
use tools::GlobalToolRegistry;

use crate::auth::{build_credential_chain, no_credentials_error, provider_hint};
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
    let (runtime_config, tool_registry) = build_runtime_plugin_state()?;
    let model = if model == "auto" {
        runtime_config
            .model()
            .map(api::resolve_model_alias)
            .unwrap_or(model)
    } else {
        model
    };
    let resolver = ModelResolver::new(&runtime_config);
    let resolved = resolver.resolve(&model)?;
    Ok(ConversationRuntime::new_with_features(
        session,
        DefaultRuntimeClient {
            runtime: tokio::runtime::Runtime::new()?,
            client: resolved.client,
            model: resolved.model,
            enable_tools,
            emit_output,
            allowed_tools: allowed_tools.clone(),
            tool_registry: tool_registry.clone(),
            progress_reporter,
            mcp_manager: Arc::clone(&mcp_manager),
            tools_disabled_by_provider: false,
        },
        CliToolExecutor::new(
            allowed_tools.clone(),
            emit_output,
            tool_registry.clone(),
            Arc::clone(&mcp_manager),
        ),
        permission_policy(permission_mode, &tool_registry),
        system_prompt,
        runtime_config.feature_config(),
    ))
}

// ---------------------------------------------------------------------------
// Model resolution pipeline
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub(crate) struct ResolvedModel {
    pub model: String,
    pub client: ProviderClient,
}

/// Single-responsibility resolver: model string → (canonical model, provider client).
///
/// Pipeline:  input → expand_shorthand → resolve_alias → build_client
pub(crate) struct ModelResolver<'a> {
    providers: &'a BTreeMap<String, CustomProviderConfig>,
    config: &'a runtime::RuntimeConfig,
}

impl<'a> ModelResolver<'a> {
    pub fn new(config: &'a runtime::RuntimeConfig) -> Self {
        Self {
            providers: config.providers(),
            config,
        }
    }

    pub fn resolve(&self, input: &str) -> Result<ResolvedModel, Box<dyn std::error::Error>> {
        let expanded = self.expand_shorthand(input)?;
        let canonical = api::resolve_model_alias(&expanded);
        match self.build_client(&canonical) {
            Ok(resolved) => Ok(resolved),
            Err(primary_err) => self.try_fallback(&canonical, primary_err),
        }
    }

    fn try_fallback(
        &self,
        primary_model: &str,
        primary_err: Box<dyn std::error::Error>,
    ) -> Result<ResolvedModel, Box<dyn std::error::Error>> {
        let fallbacks = self.config.fallback_models();
        if fallbacks.is_empty() {
            return Err(primary_err);
        }
        for fallback in fallbacks {
            let expanded = match self.expand_shorthand(fallback) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let canonical = api::resolve_model_alias(&expanded);
            if let Ok(resolved) = self.build_client(&canonical) {
                eprintln!("[info] {primary_model} unavailable, falling back to {canonical}");
                return Ok(resolved);
            }
        }
        Err(primary_err)
    }

    /// Expand bare provider names ("ollama") and "auto" into full model specs.
    fn expand_shorthand(&self, input: &str) -> Result<String, Box<dyn std::error::Error>> {
        match input {
            "auto" => self.auto_detect_model(),
            "ollama" => detect_ollama_model(self.providers)
                .ok_or_else(|| "Ollama is not running. Start it with: ollama serve".into()),
            bare if api::builtin_preset(bare).is_some()
                && api::parse_custom_provider_prefix(bare).is_none() =>
            {
                self.expand_bare_provider(bare)
            }
            other => Ok(other.to_string()),
        }
    }

    fn auto_detect_model(&self) -> Result<String, Box<dyn std::error::Error>> {
        if let Some(builtin) = api::auto_detect_default_model() {
            return Ok(builtin.to_string());
        }
        if let Some(ollama) = detect_ollama_model(self.providers) {
            return Ok(ollama);
        }
        Err(no_credentials_error().into())
    }

    fn expand_bare_provider(&self, name: &str) -> Result<String, Box<dyn std::error::Error>> {
        let lower = name.to_ascii_lowercase();
        if let Some(config) = self.providers.get(&lower) {
            if let Some(default) = &config.default_model {
                return Ok(format!("{name}/{default}"));
            }
        }
        Err(format!(
            "provider '{name}' requires a model name.\n\
             Use: codineer --model {name}/<model-name>"
        )
        .into())
    }

    fn build_client(&self, model: &str) -> Result<ResolvedModel, Box<dyn std::error::Error>> {
        if let Some((provider_name, _)) = api::parse_custom_provider_prefix(model) {
            return self.build_custom_client(model, provider_name);
        }
        self.build_builtin_client(model)
    }

    fn build_custom_client(
        &self,
        model: &str,
        provider_name: &str,
    ) -> Result<ResolvedModel, Box<dyn std::error::Error>> {
        let lower = provider_name.to_ascii_lowercase();

        let client = if let Some(config) = self.providers.get(&lower) {
            let api_key = resolve_custom_api_key(config)?;
            let mut c = OpenAiCompatClient::new_custom(&config.base_url, api_key);
            if let Some(ref v) = config.api_version {
                let q = format!("api-version={v}");
                c = c.with_endpoint_query(Some(q));
            }
            c
        } else if let Some(preset) = api::builtin_preset(&lower) {
            let api_key = resolve_preset_api_key(preset)?;
            OpenAiCompatClient::new_custom(preset.base_url, api_key)
        } else {
            return Err(format!(
                "unknown provider '{provider_name}'\n\n\
                 Built-in providers: ollama, lmstudio, openrouter, groq\n\
                 Or configure in settings.json: \
                 {{\"providers\": {{\"{provider_name}\": {{\"baseUrl\": \"...\"}}}}}}"
            )
            .into());
        };

        Ok(ResolvedModel {
            model: model.to_string(),
            client: ProviderClient::from_custom(client),
        })
    }

    fn build_builtin_client(
        &self,
        model: &str,
    ) -> Result<ResolvedModel, Box<dyn std::error::Error>> {
        let kind = api::detect_provider_kind(model);
        let chain = build_credential_chain(kind, self.config);
        let credential = chain.resolve().map_err(|err| provider_hint(model, &err))?;
        let client = ProviderClient::from_model_with_credential(model, credential)
            .map_err(|err| provider_hint(model, &err))?;
        Ok(ResolvedModel {
            model: model.to_string(),
            client,
        })
    }
}

fn resolve_custom_api_key(
    config: &CustomProviderConfig,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(key) = &config.api_key {
        return Ok(key.clone());
    }
    if let Some(env_name) = &config.api_key_env {
        let key = std::env::var(env_name).unwrap_or_default();
        if key.is_empty() {
            return Err(
                format!("provider config references env var {env_name} but it is not set").into(),
            );
        }
        return Ok(key);
    }
    Ok(String::new())
}

fn resolve_preset_api_key(
    preset: &api::BuiltinProviderPreset,
) -> Result<String, Box<dyn std::error::Error>> {
    if preset.api_key_env.is_empty() {
        return Ok(String::new());
    }
    let key = std::env::var(preset.api_key_env).unwrap_or_default();
    if key.is_empty() {
        return Err(format!(
            "provider '{}' requires {} to be set",
            preset.name, preset.api_key_env
        )
        .into());
    }
    Ok(key)
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
    tools_disabled_by_provider: bool,
}

impl DefaultRuntimeClient {
    fn effective_tools_enabled(&self) -> bool {
        self.enable_tools && !self.tools_disabled_by_provider
    }

    fn build_message_request(&self, request: &ApiRequest) -> MessageRequest {
        let use_tools = self.effective_tools_enabled();
        MessageRequest {
            model: self.model.clone(),
            max_tokens: max_tokens_for_model(&self.model),
            messages: convert_messages(&request.messages),
            system: (!request.system_prompt.is_empty()).then(|| request.system_prompt.join("\n\n")),
            tools: use_tools.then(|| {
                let mut specs = filter_tool_specs(&self.tool_registry, self.allowed_tools.as_ref());
                specs.extend(discover_mcp_tools(&self.runtime, &self.mcp_manager));
                specs
            }),
            tool_choice: use_tools.then_some(ToolChoice::Auto),
            stream: true,
        }
    }

    fn is_tool_use_error(error_msg: &str) -> bool {
        let lower = error_msg.to_ascii_lowercase();
        lower.contains("tool")
            || lower.contains("function")
            || lower.contains("unsupported parameter")
            || lower.contains("does not support")
    }
}

/// Probe local Ollama instance and pick the best coding model.
/// Returns `Some("ollama/<model>")` if Ollama is running and has models.
///
/// Base URL resolution order:
///   1. `providers.ollama.baseUrl` in settings.json
///   2. `OLLAMA_HOST` environment variable
///   3. `http://localhost:11434` (default)
fn detect_ollama_model(providers: &BTreeMap<String, CustomProviderConfig>) -> Option<String> {
    let names = query_ollama_tags(providers);
    if names.is_empty() {
        return None;
    }
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let best = pick_best_coding_model(&refs);
    Some(format!("ollama/{best}"))
}

/// Query Ollama `/api/tags` and return the list of model names.
pub(crate) fn query_ollama_tags(providers: &BTreeMap<String, CustomProviderConfig>) -> Vec<String> {
    let base = resolve_ollama_base_url(providers);
    let tags_url = format!("{}/api/tags", base.trim_end_matches('/'));
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_millis(2000))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let response = match client.get(&tags_url).send() {
        Ok(r) if r.status().is_success() => r,
        _ => return Vec::new(),
    };
    let body: serde_json::Value = match response.json() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    body.get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("name")?.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Resolve Ollama base URL from config, env, or default.
pub(crate) fn resolve_ollama_base_url(
    providers: &BTreeMap<String, CustomProviderConfig>,
) -> String {
    if let Some(config) = providers.get("ollama") {
        return config.base_url.trim_end_matches("/v1").to_string();
    }
    if let Ok(host) = std::env::var("OLLAMA_HOST") {
        let host = host.trim().trim_end_matches('/');
        if !host.is_empty() {
            if host.starts_with("http://") || host.starts_with("https://") {
                return host.to_string();
            }
            return format!("http://{host}");
        }
    }
    "http://localhost:11434".to_string()
}

/// Rank Ollama models by coding suitability.
fn pick_best_coding_model<'a>(names: &[&'a str]) -> &'a str {
    const PREFERRED: &[&str] = &[
        "qwen3-coder",
        "qwen2.5-coder",
        "qwen3",
        "deepseek-coder-v2",
        "deepseek-coder",
        "codellama",
        "starcoder2",
        "codegemma",
    ];
    for preferred in PREFERRED {
        if let Some(found) = names.iter().find(|n| n.contains(preferred)) {
            return found;
        }
    }
    names[0]
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

        let is_custom = self.client.provider_kind() == ProviderKind::Custom;
        let has_tools = message_request.tools.is_some();

        let result = self.runtime.block_on(async {
            stream_with_client(
                &self.client,
                &message_request,
                self.emit_output,
                self.progress_reporter.as_ref(),
            )
            .await
        });

        if is_custom && has_tools {
            if let Err(ref err) = result {
                if Self::is_tool_use_error(&err.to_string()) {
                    self.tools_disabled_by_provider = true;
                    eprintln!("[info] model does not support tool use; retrying without tools");
                    let fallback_request = MessageRequest {
                        tools: None,
                        tool_choice: None,
                        ..message_request
                    };
                    return self.runtime.block_on(async {
                        stream_with_client(
                            &self.client,
                            &fallback_request,
                            self.emit_output,
                            self.progress_reporter.as_ref(),
                        )
                        .await
                    });
                }
            }
        }

        result
    }
}

async fn stream_with_client(
    client: &ProviderClient,
    message_request: &MessageRequest,
    emit_output: bool,
    progress: Option<&InternalPromptProgressReporter>,
) -> Result<Vec<AssistantEvent>, RuntimeError> {
    let mut stream = client
        .stream_message(message_request)
        .await
        .map_err(|error| RuntimeError::new(error.to_string()))?;
    let mut stdout = std::io::stdout();
    let mut sink = std::io::sink();
    let out: &mut dyn Write = if emit_output { &mut stdout } else { &mut sink };
    let mut state = StreamState::new();
    while let Some(event) = stream
        .next_event()
        .await
        .map_err(|error| RuntimeError::new(error.to_string()))?
    {
        state.handle_event(event, progress, out)?;
    }

    let events = state.ensure_stop_event();
    if events
        .iter()
        .any(|event| matches!(event, AssistantEvent::MessageStop))
    {
        return Ok(events);
    }

    let response = client
        .send_message(&MessageRequest {
            stream: false,
            ..message_request.clone()
        })
        .await
        .map_err(|error| RuntimeError::new(error.to_string()))?;
    response_to_events(response, out)
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

    fn execute_tool_search(&self, input: &str) -> Result<String, ToolError> {
        let search_input: tools::ToolSearchInput = serde_json::from_str(input)
            .map_err(|e| ToolError::new(format!("invalid ToolSearch input: {e}")))?;
        let pending = self
            .mcp_manager
            .lock()
            .ok()
            .map(|guard| {
                guard
                    .unsupported_servers()
                    .iter()
                    .map(|s| s.server_name.clone())
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty());
        let output = tools::execute_tool_search_with_context(search_input, pending);
        serde_json::to_string_pretty(&output)
            .map_err(|e| ToolError::new(format!("failed to serialize ToolSearch output: {e}")))
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

        if tool_name == "ToolSearch" {
            return self.execute_tool_search(input);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn empty_providers() -> BTreeMap<String, CustomProviderConfig> {
        BTreeMap::new()
    }

    fn make_provider(base_url: &str, default_model: Option<&str>) -> CustomProviderConfig {
        CustomProviderConfig {
            base_url: base_url.to_string(),
            api_version: None,
            api_key: None,
            api_key_env: None,
            models: vec![],
            default_model: default_model.map(|s| s.to_string()),
        }
    }

    fn config_with_providers(
        providers: BTreeMap<String, CustomProviderConfig>,
    ) -> runtime::RuntimeConfig {
        let mut feature = runtime::RuntimeFeatureConfig::default();
        feature.set_providers(providers);
        runtime::RuntimeConfig::new(BTreeMap::new(), Vec::new(), feature)
    }

    // -----------------------------------------------------------------------
    // pick_best_coding_model
    // -----------------------------------------------------------------------

    #[test]
    fn pick_best_coding_model_prefers_qwen3_coder() {
        let names = vec!["llama3:8b", "qwen3-coder:30b", "deepseek-coder:6.7b"];
        assert_eq!(pick_best_coding_model(&names), "qwen3-coder:30b");
    }

    #[test]
    fn pick_best_coding_model_falls_back_to_deepseek() {
        let names = vec!["llama3:8b", "deepseek-coder-v2:16b", "mistral:7b"];
        assert_eq!(pick_best_coding_model(&names), "deepseek-coder-v2:16b");
    }

    #[test]
    fn pick_best_coding_model_falls_back_to_first_when_no_match() {
        let names = vec!["llama3:8b", "mistral:7b", "phi3:14b"];
        assert_eq!(pick_best_coding_model(&names), "llama3:8b");
    }

    #[test]
    fn pick_best_coding_model_respects_priority_order() {
        let names = vec!["codellama:13b", "qwen2.5-coder:7b", "starcoder2:3b"];
        assert_eq!(pick_best_coding_model(&names), "qwen2.5-coder:7b");
    }

    #[test]
    fn pick_best_coding_model_prefers_qwen3_over_qwen25() {
        let names = vec!["qwen2.5-coder:7b", "qwen3-coder:30b", "codellama:13b"];
        assert_eq!(pick_best_coding_model(&names), "qwen3-coder:30b");
    }

    #[test]
    fn pick_best_coding_model_selects_qwen3_over_codellama() {
        let names = vec!["codellama:13b", "qwen3:8b"];
        assert_eq!(pick_best_coding_model(&names), "qwen3:8b");
    }

    // -----------------------------------------------------------------------
    // resolve_custom_api_key
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_custom_api_key_returns_inline_key() {
        let config = CustomProviderConfig {
            base_url: "http://localhost".to_string(),
            api_version: None,
            api_key: Some("sk-test-123".to_string()),
            api_key_env: Some("SHOULD_NOT_USE".to_string()),
            models: vec![],
            default_model: None,
        };
        assert_eq!(resolve_custom_api_key(&config).unwrap(), "sk-test-123");
    }

    #[test]
    fn resolve_custom_api_key_returns_empty_when_no_key_fields() {
        let config = make_provider("http://localhost", None);
        assert_eq!(resolve_custom_api_key(&config).unwrap(), "");
    }

    #[test]
    fn resolve_custom_api_key_errors_on_missing_env_var() {
        let config = CustomProviderConfig {
            base_url: "http://localhost".to_string(),
            api_version: None,
            api_key: None,
            api_key_env: Some("__CODINEER_TEST_NONEXISTENT_KEY__".to_string()),
            models: vec![],
            default_model: None,
        };
        let err = resolve_custom_api_key(&config).unwrap_err();
        assert!(err
            .to_string()
            .contains("__CODINEER_TEST_NONEXISTENT_KEY__"));
    }

    // -----------------------------------------------------------------------
    // resolve_preset_api_key
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_preset_api_key_returns_empty_for_local_provider() {
        let preset = api::builtin_preset("ollama").unwrap();
        assert_eq!(resolve_preset_api_key(preset).unwrap(), "");
    }

    #[test]
    fn resolve_preset_api_key_errors_when_env_missing() {
        let preset = api::builtin_preset("groq").unwrap();
        let err = resolve_preset_api_key(preset).unwrap_err();
        assert!(err.to_string().contains("GROQ_API_KEY"));
    }

    // -----------------------------------------------------------------------
    // ModelResolver::expand_shorthand (via resolve)
    // -----------------------------------------------------------------------

    #[test]
    fn resolver_resolves_alias_before_building_client() {
        let config = config_with_providers(empty_providers());
        let resolver = ModelResolver::new(&config);
        // "sonnet" alias → "claude-sonnet-4-6"; will fail auth in test env but
        // the error message confirms the canonical model name was resolved.
        let err = resolver.resolve("sonnet").unwrap_err();
        assert!(
            err.to_string().contains("claude-sonnet-4-6"),
            "error should reference canonical model: {err}"
        );
    }

    #[test]
    fn resolver_passes_through_custom_prefixed_model() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "ollama".to_string(),
            make_provider("http://localhost:11434/v1", None),
        );
        let config = config_with_providers(providers);
        let resolver = ModelResolver::new(&config);
        let result = resolver.resolve("ollama/qwen3-coder:30b").unwrap();
        assert_eq!(result.model, "ollama/qwen3-coder:30b");
    }

    #[test]
    fn resolver_expands_bare_provider_with_default_model() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "groq".to_string(),
            make_provider(
                "https://api.groq.com/openai/v1",
                Some("llama-3.3-70b-versatile"),
            ),
        );
        let config = config_with_providers(providers);
        let resolver = ModelResolver::new(&config);
        let result = resolver.resolve("groq").unwrap();
        assert_eq!(result.model, "groq/llama-3.3-70b-versatile");
    }

    #[test]
    fn resolver_errors_on_bare_provider_without_default() {
        let config = config_with_providers(empty_providers());
        let resolver = ModelResolver::new(&config);
        let err = resolver.resolve("groq").unwrap_err();
        assert!(err.to_string().contains("requires a model name"));
    }

    #[test]
    fn resolver_errors_on_unknown_provider_prefix() {
        let config = config_with_providers(empty_providers());
        let resolver = ModelResolver::new(&config);
        let err = resolver.resolve("unknown-provider/some-model").unwrap_err();
        assert!(err.to_string().contains("unknown provider"));
    }

    #[test]
    fn resolver_ollama_shorthand_errors_when_not_running() {
        let config = config_with_providers(empty_providers());
        let resolver = ModelResolver::new(&config);
        let err = resolver.resolve("ollama").unwrap_err();
        assert!(err.to_string().contains("Ollama is not running"));
    }

    #[test]
    fn resolver_uses_config_over_builtin_preset() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "ollama".to_string(),
            CustomProviderConfig {
                base_url: "http://custom-ollama:11434/v1".to_string(),
                api_version: None,
                api_key: Some("custom-key".to_string()),
                api_key_env: None,
                models: vec![],
                default_model: None,
            },
        );
        let config = config_with_providers(providers);
        let resolver = ModelResolver::new(&config);
        let result = resolver.resolve("ollama/llama3:8b").unwrap();
        assert_eq!(result.model, "ollama/llama3:8b");
    }

    // -----------------------------------------------------------------------
    // is_tool_use_error
    // -----------------------------------------------------------------------

    #[test]
    fn is_tool_use_error_detects_tool_keywords() {
        assert!(DefaultRuntimeClient::is_tool_use_error(
            "tool_use is not supported"
        ));
        assert!(DefaultRuntimeClient::is_tool_use_error(
            "Function calling unavailable"
        ));
        assert!(DefaultRuntimeClient::is_tool_use_error(
            "unsupported parameter: tools"
        ));
        assert!(DefaultRuntimeClient::is_tool_use_error(
            "model does not support this feature"
        ));
    }

    #[test]
    fn is_tool_use_error_rejects_unrelated_errors() {
        assert!(!DefaultRuntimeClient::is_tool_use_error(
            "rate limit exceeded"
        ));
        assert!(!DefaultRuntimeClient::is_tool_use_error("invalid API key"));
        assert!(!DefaultRuntimeClient::is_tool_use_error(
            "connection refused"
        ));
    }

    // -----------------------------------------------------------------------
    // ModelResolver::build_custom_client with preset fallback
    // -----------------------------------------------------------------------

    #[test]
    fn resolver_uses_builtin_preset_for_lmstudio() {
        let config = config_with_providers(empty_providers());
        let resolver = ModelResolver::new(&config);
        let result = resolver.resolve("lmstudio/my-model").unwrap();
        assert_eq!(result.model, "lmstudio/my-model");
    }

    // -----------------------------------------------------------------------
    // resolve_ollama_base_url
    // -----------------------------------------------------------------------

    #[test]
    fn ollama_base_url_defaults_to_localhost() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::env::remove_var("OLLAMA_HOST");
        let providers = empty_providers();
        assert_eq!(
            resolve_ollama_base_url(&providers),
            "http://localhost:11434"
        );
    }

    #[test]
    fn ollama_base_url_from_config_takes_priority() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::env::set_var("OLLAMA_HOST", "http://env-host:9999");
        let mut providers = BTreeMap::new();
        providers.insert(
            "ollama".to_string(),
            make_provider("http://config-host:11434/v1", None),
        );
        let url = resolve_ollama_base_url(&providers);
        std::env::remove_var("OLLAMA_HOST");
        assert_eq!(url, "http://config-host:11434");
    }

    #[test]
    fn ollama_base_url_from_env_var() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::env::set_var("OLLAMA_HOST", "http://remote-host:11434");
        let providers = empty_providers();
        let url = resolve_ollama_base_url(&providers);
        std::env::remove_var("OLLAMA_HOST");
        assert_eq!(url, "http://remote-host:11434");
    }

    #[test]
    fn ollama_base_url_from_env_var_bare_host_port() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::env::set_var("OLLAMA_HOST", "192.168.1.100:11434");
        let providers = empty_providers();
        let url = resolve_ollama_base_url(&providers);
        std::env::remove_var("OLLAMA_HOST");
        assert_eq!(url, "http://192.168.1.100:11434");
    }

    #[test]
    fn ollama_base_url_from_env_var_strips_trailing_slash() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::env::set_var("OLLAMA_HOST", "http://my-server:11434/");
        let providers = empty_providers();
        let url = resolve_ollama_base_url(&providers);
        std::env::remove_var("OLLAMA_HOST");
        assert_eq!(url, "http://my-server:11434");
    }

    // -----------------------------------------------------------------------
    // try_fallback
    // -----------------------------------------------------------------------

    fn config_with_fallback(
        providers: BTreeMap<String, CustomProviderConfig>,
        fallback_models: Vec<String>,
    ) -> runtime::RuntimeConfig {
        let mut feature = runtime::RuntimeFeatureConfig::default();
        feature.set_providers(providers);
        feature.set_fallback_models(fallback_models);
        runtime::RuntimeConfig::new(BTreeMap::new(), Vec::new(), feature)
    }

    #[test]
    fn try_fallback_returns_primary_error_when_no_fallbacks() {
        let config = config_with_fallback(empty_providers(), vec![]);
        let resolver = ModelResolver::new(&config);
        let err: Box<dyn std::error::Error> = "primary failure".into();
        let result = resolver.try_fallback("sonnet", err);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("primary failure"));
    }

    #[test]
    fn try_fallback_skips_unavailable_and_returns_primary_error() {
        let config = config_with_fallback(
            empty_providers(),
            vec!["unknown-provider/model".to_string()],
        );
        let resolver = ModelResolver::new(&config);
        let err: Box<dyn std::error::Error> = "primary failure".into();
        let result = resolver.try_fallback("sonnet", err);
        assert!(result.is_err());
    }

    #[test]
    fn try_fallback_succeeds_with_available_provider() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "ollama".to_string(),
            make_provider("http://localhost:11434/v1", None),
        );
        let config = config_with_fallback(providers, vec!["ollama/qwen3-coder:30b".to_string()]);
        let resolver = ModelResolver::new(&config);
        let err: Box<dyn std::error::Error> = "primary failure".into();
        let result = resolver.try_fallback("sonnet", err);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().model, "ollama/qwen3-coder:30b");
    }

    #[test]
    fn try_fallback_tries_multiple_entries() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "ollama".to_string(),
            make_provider("http://localhost:11434/v1", None),
        );
        let config = config_with_fallback(
            providers,
            vec!["unknown/model".to_string(), "ollama/llama3:8b".to_string()],
        );
        let resolver = ModelResolver::new(&config);
        let err: Box<dyn std::error::Error> = "primary failure".into();
        let result = resolver.try_fallback("sonnet", err);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().model, "ollama/llama3:8b");
    }

    // -----------------------------------------------------------------------
    // expand_shorthand
    // -----------------------------------------------------------------------

    #[test]
    fn expand_shorthand_passes_through_model_name() {
        let config = config_with_providers(empty_providers());
        let resolver = ModelResolver::new(&config);
        let expanded = resolver.expand_shorthand("claude-sonnet-4-6").unwrap();
        assert_eq!(expanded, "claude-sonnet-4-6");
    }

    #[test]
    fn expand_shorthand_ollama_fails_gracefully() {
        let config = config_with_providers(empty_providers());
        let resolver = ModelResolver::new(&config);
        let err = resolver.expand_shorthand("ollama").unwrap_err();
        assert!(err.to_string().contains("Ollama is not running"));
    }

    // -----------------------------------------------------------------------
    // query_ollama_tags with unreachable server
    // -----------------------------------------------------------------------

    #[test]
    fn query_ollama_tags_returns_empty_on_unreachable() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "ollama".to_string(),
            make_provider("http://127.0.0.1:1/v1", None),
        );
        let result = query_ollama_tags(&providers);
        assert!(result.is_empty());
    }
}
