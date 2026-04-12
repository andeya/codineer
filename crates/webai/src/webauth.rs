use serde::{Deserialize, Serialize};
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::oneshot;

use crate::auth_store;
use crate::error::{WebAiError, WebAiResult};
use crate::provider::ProviderConfig;

/// Credentials captured from a webauth session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthCredentials {
    pub provider_id: String,
}

/// Launch a visible WebView window for the user to log in to a provider.
///
/// The window navigates to `config.start_url`. Once the user closes the
/// window (signalling login is complete), credentials are recorded.
/// HTTP cookies are automatically persisted by the system WebView engine,
/// so the hidden WebAI pages on the same domain will carry them.
pub async fn start_webauth(
    app_handle: &AppHandle,
    config: &ProviderConfig,
) -> WebAiResult<WebAuthCredentials> {
    let label = format!("webauth-{}", config.id);
    let url = WebviewUrl::External(
        config
            .start_url
            .parse()
            .map_err(|e| WebAiError::Other(anyhow::anyhow!("invalid url: {e}")))?,
    );

    let window = WebviewWindowBuilder::new(app_handle, &label, url)
        .title(format!("Login to {} — Aineer WebAuth", config.name))
        .inner_size(1024.0, 768.0)
        .build()
        .map_err(|e| WebAiError::WindowCreation(e.to_string()))?;

    let (tx, rx) = oneshot::channel::<()>();
    let tx = std::sync::Mutex::new(Some(tx));

    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
        }
    });

    let _ = rx.await;

    let creds = WebAuthCredentials {
        provider_id: config.id.clone(),
    };

    auth_store::save_credentials(&config.id, &creds)?;
    tracing::info!(provider = %config.id, "WebAuth credentials saved");

    Ok(creds)
}

/// List all providers that have saved credentials.
pub fn list_authenticated() -> Vec<String> {
    auth_store::list_authorized_providers()
}

/// Remove saved credentials for a provider.
pub fn logout(provider_id: &str) -> WebAiResult<()> {
    auth_store::remove_credentials(provider_id)
}
