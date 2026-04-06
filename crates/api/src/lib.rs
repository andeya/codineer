mod client;
mod error;
mod providers;
mod sse;
mod types;

fn default_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .unwrap_or_default()
}

pub use client::{
    oauth_token_is_expired, read_base_url, read_xai_base_url, resolve_saved_oauth_token,
    resolve_startup_auth_source, MessageStream, OAuthTokenSet, ProviderClient,
};
pub use error::ApiError;
pub use providers::codineer_provider::{AuthSource, CodineerApiClient};
pub use providers::openai_compat::{OpenAiCompatClient, OpenAiCompatConfig};
pub use providers::{
    auto_detect_default_model, builtin_preset, detect_provider_kind, list_builtin_models,
    max_tokens_for_model, parse_custom_provider_prefix, provider_kind_by_name, resolve_model_alias,
    BuiltinProviderPreset, ModelAliasEntry, ProviderKind, RetryPolicy, BUILTIN_PROVIDER_PRESETS,
};
pub use sse::{parse_frame, SseParser};
pub use types::{
    ContentBlockDelta, ContentBlockDeltaEvent, ContentBlockStartEvent, ContentBlockStopEvent,
    ImageSource, InputContentBlock, InputMessage, MessageDelta, MessageDeltaEvent, MessageRequest,
    MessageResponse, MessageStartEvent, MessageStopEvent, OutputContentBlock, StreamEvent,
    ToolChoice, ToolDefinition, ToolResultContentBlock, Usage,
};
