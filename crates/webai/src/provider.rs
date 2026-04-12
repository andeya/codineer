use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::error::WebAiResult;
use crate::page::WebAiPage;

/// Metadata for a model offered by a web AI provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub default: bool,
}

/// Result of parsing a complete (non-streaming) response.
#[derive(Debug, Clone)]
pub struct StreamResult {
    pub text: String,
    pub thinking_text: String,
}

/// Configuration for a WebAI provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub start_url: String,
    pub host_key: String,
    pub models: Vec<ModelInfo>,
}

/// Core trait that every web AI provider must implement.
///
/// The provider operates on a [`WebAiPage`] that is already loaded on the
/// provider's domain and carries authentication cookies.  Methods receive the
/// page as a parameter so the manager can pool/reuse pages.
///
/// This mirrors TFG's `WebProviderClient` but is Rust-native: the JS
/// automation code lives as string constants inside each implementation and
/// is dispatched via `page.evaluate()`.
#[async_trait]
pub trait WebProviderClient: Send + Sync {
    /// Unique identifier (e.g. `"claude"`, `"chatgpt"`).
    fn provider_id(&self) -> &str;

    /// Provider configuration.
    fn config(&self) -> &ProviderConfig;

    /// One-time initialization after the page loads (e.g. fetch org ID).
    async fn init(&self, page: &WebAiPage) -> WebAiResult<()> {
        let _ = page;
        Ok(())
    }

    /// Send a message and get a streaming response.
    ///
    /// Returns an `mpsc::Receiver` that yields content chunks as they arrive.
    /// Implementations should use `page.evaluate_streaming()` or
    /// `page.evaluate()` + parse the response.
    async fn send_message(
        &self,
        page: &WebAiPage,
        message: &str,
        model: &str,
    ) -> WebAiResult<mpsc::Receiver<String>>;

    /// List models available for this provider.
    fn list_models(&self) -> Vec<ModelInfo> {
        self.config().models.clone()
    }

    /// Check if the current session (cookies) is valid.
    async fn check_session(&self, page: &WebAiPage) -> WebAiResult<bool> {
        let _ = page;
        Ok(true)
    }
}
