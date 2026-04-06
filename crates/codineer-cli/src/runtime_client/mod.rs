mod messages;
mod model;
mod permission;
mod stream;
mod tool_executor;

use std::sync::Arc;

use api::{MessageRequest, ProviderClient, ProviderKind, ToolChoice};
use runtime::{ApiClient, ApiRequest, AssistantEvent, ConversationRuntime, RuntimeError};
use tools::GlobalToolRegistry;

use crate::cli::{discover_mcp_tools, filter_tool_specs, AllowedToolSet, SharedMcpManager};
use crate::progress::InternalPromptProgressReporter;
use crate::{build_runtime_plugin_state, max_tokens_for_model};

pub(crate) use messages::convert_messages;
#[allow(unused_imports)]
pub(crate) use model::{query_ollama_tags, resolve_ollama_base_url, ModelResolver, ResolvedModel};
pub(crate) use permission::{permission_policy, CliPermissionPrompter};
#[allow(unused_imports)]
pub(crate) use stream::{push_output_block, response_to_events, write_flush};
pub(crate) use tool_executor::CliToolExecutor;

pub(crate) struct RuntimeParams {
    pub(crate) session: runtime::Session,
    pub(crate) model: String,
    pub(crate) system_prompt: Vec<String>,
    pub(crate) enable_tools: bool,
    pub(crate) emit_output: bool,
    pub(crate) allowed_tools: Option<AllowedToolSet>,
    pub(crate) permission_mode: runtime::PermissionMode,
    pub(crate) progress_reporter: Option<InternalPromptProgressReporter>,
    pub(crate) mcp_manager: SharedMcpManager,
}

pub(crate) struct RuntimeBuildResult {
    pub runtime: ConversationRuntime<DefaultRuntimeClient, CliToolExecutor>,
    pub resolved_model: String,
    pub model_aliases: std::collections::BTreeMap<String, String>,
}

pub(crate) fn build_runtime(
    params: RuntimeParams,
) -> Result<RuntimeBuildResult, Box<dyn std::error::Error>> {
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
            .map(|m| api::resolve_model_alias(m, runtime_config.model_aliases()))
            .unwrap_or(model)
    } else {
        api::resolve_model_alias(&model, runtime_config.model_aliases())
    };
    let resolver = ModelResolver::new(&runtime_config);
    let resolved = resolver.resolve(&model)?;
    let resolved_model = resolved.model.clone();
    let shared_runtime = Arc::new(tokio::runtime::Runtime::new()?);
    let runtime = ConversationRuntime::new_with_features(
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
        },
        CliToolExecutor::new(
            allowed_tools.clone(),
            emit_output,
            tool_registry.clone(),
            Arc::clone(&mcp_manager),
            Arc::clone(&shared_runtime),
        ),
        permission_policy(permission_mode, &tool_registry),
        system_prompt,
        runtime_config.feature_config(),
    );
    let model_aliases = runtime_config.model_aliases().clone();
    Ok(RuntimeBuildResult {
        runtime,
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

impl ApiClient for DefaultRuntimeClient {
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
                        ..message_request
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

        result
    }
}

#[cfg(test)]
mod tests;
