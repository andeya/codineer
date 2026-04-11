mod messages;
mod model;
mod permission;
mod stream;
mod tool_executor;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aineer_api::{
    CacheControl, CacheScope, CacheType, MessageRequest, ProviderClient, ProviderKind, ToolChoice,
    ToolDefinition,
};
use aineer_engine::{
    ApiClient, ApiRequest, AssistantEvent, CacheLock, ConversationRuntime, HookDispatcher,
    RuntimeError,
};
use aineer_tools::GlobalToolRegistry;

use crate::error::CliResult;
use crate::tracing_observer::TracingObserver;

pub(crate) type CliObserver = (HookDispatcher, TracingObserver);

use crate::cli::{discover_mcp_tools, AllowedToolSet, SharedMcpManager};
use crate::progress::InternalPromptProgressReporter;
use crate::{build_runtime_plugin_state, max_tokens_for_model};

pub(crate) use messages::convert_messages;
#[allow(unused_imports)]
pub(crate) use model::{
    query_ollama_tags, resolve_custom_api_key, resolve_ollama_base_url, resolve_preset_api_key,
    ModelResolver,
};
pub(crate) use permission::{permission_policy, CliPermissionPrompter};
#[allow(unused_imports)]
pub(crate) use stream::{push_output_block, response_to_events, write_flush};
pub(crate) use tool_executor::CliToolExecutor;

pub(crate) struct RuntimeParams {
    pub(crate) session: aineer_engine::Session,
    pub(crate) model: String,
    pub(crate) system_prompt: Vec<aineer_api::SystemBlock>,
    pub(crate) enable_tools: bool,
    pub(crate) emit_output: bool,
    pub(crate) allowed_tools: Option<AllowedToolSet>,
    pub(crate) permission_mode: aineer_engine::PermissionMode,
    pub(crate) progress_reporter: Option<InternalPromptProgressReporter>,
    pub(crate) mcp_manager: SharedMcpManager,
}

pub(crate) struct RuntimeBuildResult {
    pub runtime: ConversationRuntime<DefaultRuntimeClient, CliToolExecutor, CliObserver>,
    /// Tokio runtime shared by [`DefaultRuntimeClient`] and [`CliToolExecutor`] for this build.
    pub tokio_runtime: Arc<tokio::runtime::Runtime>,
    pub resolved_model: String,
    pub model_aliases: std::collections::BTreeMap<String, String>,
}

pub(crate) fn build_runtime(params: RuntimeParams) -> CliResult<RuntimeBuildResult> {
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
            .map(|m| aineer_api::resolve_model_alias(m, runtime_config.model_aliases()))
            .unwrap_or(model)
    } else {
        aineer_api::resolve_model_alias(&model, runtime_config.model_aliases())
    };
    let resolver = ModelResolver::new(&runtime_config);
    let resolved = resolver.resolve(&model)?;
    let resolved_model = resolved.model.clone();

    let fallback_models: Vec<(String, ProviderClient)> = runtime_config
        .fallback_models()
        .iter()
        .filter_map(|fb| {
            let expanded = resolver.expand_shorthand(fb).ok()?;
            let canonical =
                aineer_api::resolve_model_alias(&expanded, runtime_config.model_aliases());
            if canonical == resolved_model {
                return None;
            }
            let r = resolver.resolve(&canonical).ok()?;
            Some((r.model, r.client))
        })
        .collect();

    let shared_runtime = Arc::new(tokio::runtime::Runtime::new()?);
    let cached_mcp_tools: Arc<Mutex<Option<Vec<ToolDefinition>>>> = Arc::new(Mutex::new(None));
    let activated_tools: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let observer: CliObserver = (
        HookDispatcher::from_feature_config(runtime_config.feature_config()),
        TracingObserver,
    );
    let runtime = ConversationRuntime::new(
        session,
        DefaultRuntimeClient {
            runtime: Arc::clone(&shared_runtime),
            client: resolved.client,
            model: resolved.model,
            enable_tools,
            emit_output,
            allowed_tools: allowed_tools.clone(),
            tool_registry: tool_registry.clone(),
            progress_reporter,
            mcp_manager: Arc::clone(&mcp_manager),
            tools_disabled_by_provider: false,
            fallback_models,
            cached_mcp_tools: Arc::clone(&cached_mcp_tools),
            activated_tools: Arc::clone(&activated_tools),
            cache_lock: CacheLock::default(),
        },
        CliToolExecutor::new(
            allowed_tools.clone(),
            emit_output,
            tool_registry.clone(),
            Arc::clone(&mcp_manager),
            Arc::clone(&shared_runtime),
            Arc::clone(&cached_mcp_tools),
            Arc::clone(&activated_tools),
        ),
        permission_policy(
            permission_mode,
            &tool_registry,
            runtime_config.permission_rules(),
        ),
        system_prompt,
        observer,
    );
    let model_aliases = runtime_config.model_aliases().clone();
    Ok(RuntimeBuildResult {
        runtime,
        tokio_runtime: shared_runtime,
        resolved_model,
        model_aliases,
    })
}

pub(crate) struct DefaultRuntimeClient {
    runtime: Arc<tokio::runtime::Runtime>,
    client: ProviderClient,
    model: String,
    enable_tools: bool,
    emit_output: bool,
    allowed_tools: Option<AllowedToolSet>,
    tool_registry: GlobalToolRegistry,
    progress_reporter: Option<InternalPromptProgressReporter>,
    mcp_manager: SharedMcpManager,
    tools_disabled_by_provider: bool,
    fallback_models: Vec<(String, ProviderClient)>,
    /// Session-scoped MCP tool list (shared with [`CliToolExecutor`]). Cleared when
    /// [`CacheLock`] expires so MCP reconnects can refresh definitions.
    cached_mcp_tools: Arc<Mutex<Option<Vec<ToolDefinition>>>>,
    /// Extended-tier and MCP tool names activated via [`ToolSearch`] for this session.
    activated_tools: Arc<Mutex<HashSet<String>>>,
    /// Aligns tool cache refresh with a 1h window so TTL expiry does not
    /// invalidate the MCP tool list mid-session.
    cache_lock: CacheLock,
}

impl DefaultRuntimeClient {
    fn effective_tools_enabled(&self) -> bool {
        self.enable_tools && !self.tools_disabled_by_provider
    }

    fn cache_control_for_remaining(remaining: Duration) -> CacheControl {
        CacheControl {
            kind: CacheType::Ephemeral,
            ttl: Some(Self::format_tool_cache_ttl_hint(remaining)),
            scope: Some(CacheScope::Global),
        }
    }

    /// Anthropic-style `ttl` string for the last tool definition (`5m`, `1h`, …).
    fn format_tool_cache_ttl_hint(remaining: Duration) -> String {
        let secs = remaining.as_secs().max(1);
        if secs >= 3600 {
            let h = secs.div_ceil(3600);
            format!("{h}h")
        } else {
            let m = secs.div_ceil(60).max(1);
            format!("{m}m")
        }
    }

    fn build_message_request(&mut self, request: &ApiRequest) -> MessageRequest {
        let use_tools = self.effective_tools_enabled();
        let tools = if use_tools {
            if !self.cache_lock.is_valid() {
                if let Ok(mut guard) = self.cached_mcp_tools.lock() {
                    *guard = None;
                }
                self.cache_lock = CacheLock::default();
            }
            let mcp = {
                let mut guard = self
                    .cached_mcp_tools
                    .lock()
                    .expect("MCP tool cache mutex poisoned");
                guard
                    .get_or_insert_with(|| discover_mcp_tools(&self.runtime, &self.mcp_manager))
                    .clone()
            };
            let activated = self
                .activated_tools
                .lock()
                .expect("activated_tools mutex poisoned");
            let mut specs = self
                .tool_registry
                .definitions_for_lazy_prompt(self.allowed_tools.as_ref(), Some(&*activated));
            specs.extend(mcp.into_iter().filter(|def| {
                self.allowed_tools
                    .as_ref()
                    .is_none_or(|allowed| allowed.contains(&def.name))
                    && activated.contains(&def.name)
            }));
            if let Some(last) = specs.last_mut() {
                last.cache_control = Some(Self::cache_control_for_remaining(
                    self.cache_lock.remaining(),
                ));
            }
            Some(specs)
        } else {
            None
        };
        let model = request.model_override.as_ref().unwrap_or(&self.model);
        MessageRequest {
            model: model.clone(),
            max_tokens: request
                .max_output_tokens
                .unwrap_or_else(|| max_tokens_for_model(model)),
            messages: convert_messages(&request.messages),
            system: (!request.system_prompt.is_empty()).then(|| request.system_prompt.clone()),
            tools,
            tool_choice: use_tools.then_some(ToolChoice::Auto),
            stream: true,
            thinking: None,
            gemini_cached_content: None,
        }
    }

    fn is_tool_use_error(error_msg: &str) -> bool {
        let lower = error_msg.to_ascii_lowercase();
        lower.contains("tool")
            || lower.contains("function")
            || lower.contains("unsupported parameter")
            || lower.contains("does not support")
    }

    /// Errors that indicate the current model/provider is temporarily or
    /// permanently unable to serve requests — worth trying a different model.
    fn should_try_model_fallback(error_msg: &str) -> bool {
        let lower = error_msg.to_ascii_lowercase();
        lower.contains("429")
            || lower.contains("rate limit")
            || lower.contains("quota")
            || lower.contains("exceeded")
            || lower.contains("model") && lower.contains("not available")
            || lower.contains("capacity")
            || lower.contains("overloaded")
    }
}

impl ApiClient for DefaultRuntimeClient {
    fn active_model(&self) -> &str {
        &self.model
    }

    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
        if let Some(progress_reporter) = &self.progress_reporter {
            progress_reporter.mark_model_phase();
        }
        let message_request = self.build_message_request(&request);

        let is_custom = self.client.provider_kind() == ProviderKind::Custom;
        let has_tools = message_request.tools.is_some();

        let result = self.runtime.block_on(async {
            stream::stream_with_client(
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
                        ..message_request.clone()
                    };
                    return self.runtime.block_on(async {
                        stream::stream_with_client(
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

        // Try fallback models on rate-limit / model-unavailable errors
        if let Err(ref primary_err) = result {
            if !self.fallback_models.is_empty()
                && Self::should_try_model_fallback(&primary_err.to_string())
            {
                let primary_model = self.model.clone();
                for (fb_model, fb_client) in &self.fallback_models {
                    if fb_model == &primary_model {
                        continue;
                    }
                    eprintln!(
                        "[info] {primary_model} unavailable ({err}), trying fallback {fb_model}",
                        err = primary_err
                    );
                    let fb_request = MessageRequest {
                        model: fb_model.clone(),
                        max_tokens: max_tokens_for_model(fb_model),
                        ..message_request.clone()
                    };
                    let fb_result = self.runtime.block_on(async {
                        stream::stream_with_client(
                            fb_client,
                            &fb_request,
                            self.emit_output,
                            self.progress_reporter.as_ref(),
                        )
                        .await
                    });
                    if fb_result.is_ok() {
                        eprintln!("[info] switched to fallback model {fb_model}");
                        self.model = fb_model.clone();
                        self.client = fb_client.clone();
                        return fb_result;
                    }
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests;
