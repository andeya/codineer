use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum WebAiError {
    #[error("WebView eval failed: {0}")]
    Eval(String),

    #[error("JavaScript execution error: {0}")]
    JsError(String),

    #[error("Timed out after {0:?} waiting for WebView response")]
    Timeout(Duration),

    #[error("Event listener channel closed unexpectedly")]
    ChannelClosed,

    #[error("Failed to create WebView window: {0}")]
    WindowCreation(String),

    #[error("Provider not authenticated: {provider_id}")]
    NotAuthenticated { provider_id: String },

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Stream ended unexpectedly")]
    StreamEnded,

    #[error("Deserialization failed: {0}")]
    Deserialize(#[from] serde_json::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type WebAiResult<T> = Result<T, WebAiError>;
