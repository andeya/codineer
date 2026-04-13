pub mod auth_store;
pub mod engine;
pub mod error;
pub mod page;
pub mod page_manager;
pub mod provider;
pub mod providers;
pub mod sse_parser;
pub mod tool_calling;
pub mod webauth;

pub use engine::{OpenAiStreamResult, WebAiEngine};
pub use error::{WebAiError, WebAiResult};
pub use page::WebAiPage;
pub use page_manager::WebAiPageManager;
pub use provider::{ModelInfo, ProviderConfig, StreamResult, WebProviderClient};

/// User-Agent string sent by all WebAI WebView windows.
///
/// Using a modern Chrome UA ensures provider websites serve compatible
/// content (TLS handshake, JS bundle selection, feature flags).
pub const WEBVIEW_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
