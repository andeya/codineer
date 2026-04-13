use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::oneshot;

use crate::auth_store;
use crate::error::{WebAiError, WebAiResult};
use crate::provider::ProviderConfig;

/// Credentials captured from a webauth session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthCredentials {
    pub provider_id: String,
}

/// JS injected into every webauth page.
///
/// Two responsibilities:
/// 1. **Compatibility check** — probes for named capture groups
///    (`(?<name>...)`) which require Safari 16.4+ / macOS 13+.  When the
///    check fails an overlay replaces the (blank) page with a friendly
///    message, the login URL, and a copy-to-clipboard button.
/// 2. **Banner** — when the page IS compatible, a bottom bar reminds the
///    user to close the window after login.
const WEBAUTH_INIT_JS: &str = r#"
(function(){
  if(window.__aineer_init) return;
  window.__aineer_init=true;

  /* ── feature detection ── */
  var compat=true;
  try{new RegExp('(?<t>a)');}catch(e){compat=false;}

  /* ── compatibility fallback overlay ── */
  function showFallback(){
    if(!document.body){setTimeout(showFallback,150);return;}
    if(document.getElementById('__aineer_fb'))return;
    var o=document.createElement('div');o.id='__aineer_fb';
    o.style.cssText='position:fixed;inset:0;z-index:2147483647;background:#0f0f10;color:#e4e4e7;display:flex;align-items:center;justify-content:center;font-family:-apple-system,BlinkMacSystemFont,sans-serif;padding:2rem;';
    var c=document.createElement('div');
    c.style.cssText='max-width:480px;width:100%;text-align:center;background:#18181b;border:1px solid #27272a;border-radius:16px;padding:2.5rem 2rem;';

    var icon=document.createElement('div');
    icon.style.cssText='font-size:2.5rem;margin-bottom:1rem;';
    icon.textContent='\u26A0\uFE0F';

    var h=document.createElement('h2');
    h.style.cssText='font-size:1.15rem;margin-bottom:0.75rem;color:#fafafa;';
    h.textContent='Browser Compatibility Issue';

    var p1=document.createElement('p');
    p1.style.cssText='font-size:0.8rem;color:#a1a1aa;line-height:1.6;margin-bottom:1.25rem;';
    p1.textContent='This website uses modern JavaScript features that require macOS 13 (Ventura) or later. The built-in browser on your system cannot render this page.';

    var p2=document.createElement('p');
    p2.style.cssText='font-size:0.75rem;color:#71717a;margin-bottom:1rem;';
    p2.textContent='You can copy the URL below and open it in Safari or Chrome to log in manually:';

    var url=document.createElement('div');
    url.style.cssText='font-size:0.7rem;color:#71717a;background:#09090b;border:1px solid #27272a;border-radius:8px;padding:0.6rem 1rem;margin-bottom:1.25rem;word-break:break-all;font-family:SF Mono,Monaco,monospace;user-select:all;cursor:text;';
    url.textContent=location.href;

    var btns=document.createElement('div');
    btns.style.cssText='display:flex;gap:0.75rem;justify-content:center;flex-wrap:wrap;';

    var copyBtn=document.createElement('button');
    copyBtn.textContent='\uD83D\uDCCB  Copy URL';
    copyBtn.style.cssText='background:#3b82f6;color:#fff;border:none;padding:8px 20px;border-radius:8px;font-size:0.8rem;font-weight:600;cursor:pointer;transition:background 0.15s;';
    copyBtn.onmouseenter=function(){copyBtn.style.background='#2563eb';};
    copyBtn.onmouseleave=function(){copyBtn.style.background='#3b82f6';};
    copyBtn.onclick=function(){
      if(navigator.clipboard&&navigator.clipboard.writeText){
        navigator.clipboard.writeText(location.href).then(function(){copyBtn.textContent='\u2705  Copied!';setTimeout(function(){copyBtn.textContent='\uD83D\uDCCB  Copy URL';},2000);});
      }else{
        var ta=document.createElement('textarea');ta.value=location.href;ta.style.cssText='position:fixed;left:-9999px;';document.body.appendChild(ta);ta.select();document.execCommand('copy');document.body.removeChild(ta);
        copyBtn.textContent='\u2705  Copied!';setTimeout(function(){copyBtn.textContent='\uD83D\uDCCB  Copy URL';},2000);
      }
    };

    var closeBtn=document.createElement('button');
    closeBtn.textContent='\u2714  Close';
    closeBtn.style.cssText='background:#27272a;color:#e4e4e7;border:1px solid #3f3f46;padding:8px 20px;border-radius:8px;font-size:0.8rem;font-weight:600;cursor:pointer;transition:background 0.15s;';
    closeBtn.onmouseenter=function(){closeBtn.style.background='#3f3f46';};
    closeBtn.onmouseleave=function(){closeBtn.style.background='#27272a';};
    closeBtn.onclick=function(){window.close();};

    btns.appendChild(copyBtn);btns.appendChild(closeBtn);

    var note=document.createElement('p');
    note.style.cssText='font-size:0.65rem;color:#52525b;margin-top:1.25rem;line-height:1.5;';
    note.textContent='After logging in your browser, you can close this window. If cookies are shared with the system browser, your session may still be recognized. For full compatibility, please upgrade to macOS 13+.';

    c.appendChild(icon);c.appendChild(h);c.appendChild(p1);c.appendChild(p2);c.appendChild(url);c.appendChild(btns);c.appendChild(note);
    o.appendChild(c);document.body.appendChild(o);
  }

  /* ── normal login banner ── */
  function showBanner(){
    if(!document.body){setTimeout(showBanner,200);return;}
    if(document.getElementById('__aineer_bar'))return;
    var b=document.createElement('div');b.id='__aineer_bar';
    b.style.cssText='position:fixed;bottom:0;left:0;right:0;z-index:2147483647;background:linear-gradient(135deg,#1e293b,#0f172a);color:#e2e8f0;padding:10px 20px;display:flex;align-items:center;justify-content:space-between;font-family:-apple-system,BlinkMacSystemFont,sans-serif;font-size:13px;box-shadow:0 -2px 12px rgba(0,0,0,0.4);border-top:1px solid rgba(255,255,255,0.08);gap:12px;';
    var t=document.createElement('span');t.style.cssText='flex:1;opacity:0.9;';
    t.textContent='\u2139\uFE0F  Log in to your account, then close this window or click Done.';
    var d=document.createElement('button');d.textContent='\u2714  Done';
    d.style.cssText='background:#16a34a;color:#fff;border:none;padding:6px 20px;border-radius:6px;font-size:13px;font-weight:600;cursor:pointer;white-space:nowrap;transition:background 0.15s;';
    d.onmouseenter=function(){d.style.background='#15803d';};
    d.onmouseleave=function(){d.style.background='#16a34a';};
    d.onclick=function(){window.close();};
    b.appendChild(t);b.appendChild(d);document.body.appendChild(b);
  }

  /* ── entry point ── */
  if(!compat){
    window.onerror=function(msg){
      if(msg&&(msg.indexOf('SyntaxError')!==-1||msg.indexOf('regular expression')!==-1)){
        showFallback();return true;
      }
    };
    showFallback();
  } else {
    showBanner();
    var _push=history.pushState;
    history.pushState=function(){_push.apply(history,arguments);showBanner();};
  }
})();
"#;

/// Launch the WebAuth flow.
///
/// Opens the provider login page **directly inside a visible WKWebView window**.
/// This is critical because the hidden `webai-*` pages created by
/// [`WebAiPageManager`] share the same `WKWebsiteDataStore` — cookies set
/// during login are automatically available to them.
///
/// Using the system browser would NOT work: Safari/Chrome have a separate
/// cookie jar from WKWebView.
pub async fn start_webauth(
    app_handle: &AppHandle,
    config: &ProviderConfig,
) -> WebAiResult<WebAuthCredentials> {
    let label = format!("webauth-{}", config.id);

    // If a webauth window for this provider already exists, bring it to front.
    if let Some(existing) = app_handle.get_webview_window(&label) {
        let _ = existing.set_focus();
        return Err(WebAiError::Other(anyhow::anyhow!(
            "Login window for {} is already open",
            config.name
        )));
    }

    let url: url::Url = config
        .start_url
        .parse()
        .map_err(|e| WebAiError::WindowCreation(format!("invalid URL: {e}")))?;

    let window = WebviewWindowBuilder::new(app_handle, &label, WebviewUrl::External(url))
        .title(format!("Login to {} — Aineer", config.name))
        .inner_size(1024.0, 768.0)
        .resizable(true)
        .center()
        .initialization_script(WEBAUTH_INIT_JS)
        .build()
        .map_err(|e| WebAiError::WindowCreation(e.to_string()))?;

    // Wait for the user to close the window (signals login completion).
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

    // Record that the user has authenticated with this provider.
    // The actual session cookies live in WKWebView's shared WKWebsiteDataStore
    // and are automatically available to hidden webai-* pages.
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
