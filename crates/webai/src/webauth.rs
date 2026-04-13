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

/// Build JS that injects CSS + body content into an about:blank page.
/// Avoids `document.write()` (causes init-script recursion) and full
/// `documentElement.innerHTML` (creates invalid nested `<html>`).
fn build_inject_js(provider_name: &str, login_url: &str) -> String {
    let css = r#"*{margin:0;padding:0;box-sizing:border-box}body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:#0f0f10;color:#e4e4e7;display:flex;align-items:center;justify-content:center;height:100vh;padding:2rem}.card{max-width:420px;width:100%;text-align:center;background:#18181b;border:1px solid #27272a;border-radius:12px;padding:2.5rem 2rem}h2{font-size:1.25rem;margin-bottom:1.5rem;color:#fafafa}.url{display:inline-block;font-size:.75rem;color:#71717a;background:#09090b;border:1px solid #27272a;border-radius:6px;padding:.4rem .8rem;margin-bottom:1.5rem;word-break:break-all;font-family:'SF Mono',Monaco,monospace}.btn{display:inline-block;padding:.65rem 1.5rem;border-radius:8px;font-size:.85rem;font-weight:600;cursor:pointer;border:none;transition:all .15s;width:100%}.btn-success{background:#16a34a;color:#fff}.btn-success:hover{background:#15803d}.step{display:flex;align-items:flex-start;gap:.75rem;text-align:left;margin-bottom:.75rem}.step-num{flex-shrink:0;width:1.5rem;height:1.5rem;border-radius:50%;background:#27272a;color:#a1a1aa;font-size:.7rem;font-weight:700;display:flex;align-items:center;justify-content:center}.step-text{font-size:.8rem;color:#a1a1aa;line-height:1.5}.step-text b{color:#e4e4e7}.note{font-size:.7rem;color:#52525b;margin-top:1rem}"#;

    let body_html = format!(
        r#"<div class="card"><h2>Login to {provider_name}</h2><div style="margin-bottom:1.25rem"><div class="step"><span class="step-num">1</span><span class="step-text">Your browser has opened the login page</span></div><div class="step"><span class="step-num">2</span><span class="step-text">Complete the login in your browser</span></div><div class="step"><span class="step-num">3</span><span class="step-text">Return here and close this window</span></div></div><div class="url">{login_url}</div><button class="btn btn-success" onclick="window.close()">Done — Close Window</button><p class="note">Closing this window will save your login status.</p></div>"#
    );

    format!(
        r#"(function(){{var s=document.createElement('style');s.textContent={css};document.head.appendChild(s);document.body.innerHTML={body};document.title={title};}})();"#,
        css = serde_json::to_string(css).unwrap_or_default(),
        body = serde_json::to_string(&body_html).unwrap_or_default(),
        title = serde_json::to_string(&format!("Login to {provider_name}")).unwrap_or_default(),
    )
}

/// Launch the WebAuth flow.
///
/// Opens the provider login page in the system default browser (which supports
/// all modern JS) and shows a lightweight helper window instructing the user to
/// close it when done.  Credentials are recorded when the helper window closes.
pub async fn start_webauth(
    app_handle: &AppHandle,
    config: &ProviderConfig,
) -> WebAiResult<WebAuthCredentials> {
    // Open the login URL in the system default browser
    let _ = std::process::Command::new("open")
        .arg(&config.start_url)
        .spawn();

    let label = format!("webauth-{}", config.id);

    let window = WebviewWindowBuilder::new(
        app_handle,
        &label,
        WebviewUrl::External("about:blank".parse().unwrap()),
    )
    .title(format!("Login to {} — Aineer WebAuth", config.name))
    .inner_size(480.0, 440.0)
    .resizable(false)
    .build()
    .map_err(|e| WebAiError::WindowCreation(e.to_string()))?;

    // Inject CSS + body content via eval (safe — no document.write, no recursion)
    let inject_js = build_inject_js(&config.name, &config.start_url);
    let _ = window.eval(&inject_js);

    let (tx, rx) = oneshot::channel::<()>();
    let tx = std::sync::Mutex::new(Some(tx));

    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            if let Some(sender) = tx.lock().unwrap().take() {
                let _ = sender.send(());
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
