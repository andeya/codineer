use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("Channel '{0}' not configured")]
    NotConfigured(String),
    #[error("API error: {0}")]
    Api(String),
    #[error("Authentication failed for channel '{0}'")]
    AuthFailed(String),
}

/// Unified message from any channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMessage {
    pub source: ChannelSource,
    pub sender: String,
    pub text: String,
    pub attachments: Vec<Attachment>,
    pub reply_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelSource {
    Desktop,
    Feishu,
    WeChat,
    WhatsApp,
    Gateway,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

/// Trait for channel adapters
#[async_trait::async_trait]
pub trait ChannelAdapter: Send + Sync {
    fn channel_type(&self) -> ChannelSource;
    async fn send_message(&self, to: &str, message: &str) -> Result<(), ChannelError>;
    async fn send_approval_request(&self, to: &str, description: &str) -> Result<(), ChannelError>;
}
