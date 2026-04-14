use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Listener, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::oneshot;

use crate::auth_store;
use crate::error::{WebAiError, WebAiResult};
use crate::provider::ProviderConfig;

/// Credentials captured from a webauth session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebAuthCredentials {
    pub provider_id: String,
    /// Whether expected session cookies were found in the WebView cookie store
    /// at the moment the user closed the login window.
    #[serde(default = "default_true")]
    pub session_verified: bool,
}

fn default_true() -> bool {
    true
}

/// JS injected into every webauth page.
///
/// Three responsibilities:
/// 1. **Compatibility check** — probes for named capture groups
///    (`(?<name>...)`) which require Safari 16.4+ / macOS 13+.  When the
///    check fails an overlay replaces the (blank) page with a friendly
///    message, the login URL, and a copy-to-clipboard button.
/// 2. **Crash recovery** — a global `onerror` handler catches `SyntaxError`
///    from the site's own JS bundle (e.g. when the site uses newer regex
///    features the WebView engine doesn't support), and a 5-second blank-page
///    watchdog triggers the fallback overlay if the site fails to render.
/// 3. **Banner** — when the page IS compatible, a bottom bar reminds the
///    user to close the window after login.
const WEBAUTH_INIT_JS: &str = r#"
(function(){
  if(window.__aineer_init) return;
  window.__aineer_init=true;

  /* ── feature detection ── */
  var compat=true;
  try{new RegExp('(?<t>a)');}catch(e){compat=false;}

  /* ── compatibility fallback overlay with cookie paste ── */
  function showFallback(){
    if(!document.body){setTimeout(showFallback,150);return;}
    if(document.getElementById('__aineer_fb'))return;
    var o=document.createElement('div');o.id='__aineer_fb';
    o.style.cssText='position:fixed;inset:0;z-index:2147483647;background:#0f0f10;color:#e4e4e7;display:flex;align-items:center;justify-content:center;font-family:-apple-system,BlinkMacSystemFont,sans-serif;padding:2rem;overflow-y:auto;';
    var c=document.createElement('div');
    c.style.cssText='max-width:520px;width:100%;text-align:center;background:#18181b;border:1px solid #27272a;border-radius:16px;padding:2rem 1.75rem;';

    var icon=document.createElement('div');
    icon.style.cssText='font-size:2rem;margin-bottom:0.75rem;';
    icon.textContent='\u26A0\uFE0F';

    var h=document.createElement('h2');
    h.style.cssText='font-size:1.1rem;margin-bottom:0.5rem;color:#fafafa;';
    h.textContent='Browser Compatibility Issue';

    var p1=document.createElement('p');
    p1.style.cssText='font-size:0.78rem;color:#a1a1aa;line-height:1.55;margin-bottom:1rem;';
    p1.textContent='This page requires macOS 13+ to render. You can still log in by pasting cookies from your system browser:';

    /* step-by-step guide */
    var steps=document.createElement('div');
    steps.style.cssText='text-align:left;margin-bottom:1rem;';
    var stepData=[
      ['1','Copy the URL below and open it in Safari or Chrome'],
      ['2','Log in to your account on that page'],
      ['3','Open DevTools (F12), go to Console tab'],
      ['4','Run: document.cookie  and copy the output'],
      ['5','Paste the result below and click Apply']
    ];
    for(var i=0;i<stepData.length;i++){
      var row=document.createElement('div');
      row.style.cssText='display:flex;align-items:flex-start;gap:0.6rem;margin-bottom:0.4rem;';
      var num=document.createElement('span');
      num.style.cssText='flex-shrink:0;width:1.3rem;height:1.3rem;border-radius:50%;background:#27272a;color:#a1a1aa;font-size:0.6rem;font-weight:700;display:flex;align-items:center;justify-content:center;margin-top:0.1rem;';
      num.textContent=stepData[i][0];
      var txt=document.createElement('span');
      txt.style.cssText='font-size:0.72rem;color:#a1a1aa;line-height:1.4;';
      if(i===3){
        txt.innerHTML='Run: <code style="background:#09090b;padding:1px 5px;border-radius:3px;font-family:SF Mono,Monaco,monospace;color:#e4e4e7;font-size:0.68rem;">document.cookie</code> and copy the output';
      }else{
        txt.textContent=stepData[i][1];
      }
      row.appendChild(num);row.appendChild(txt);steps.appendChild(row);
    }

    /* URL display + copy */
    var url=document.createElement('div');
    url.style.cssText='font-size:0.68rem;color:#71717a;background:#09090b;border:1px solid #27272a;border-radius:8px;padding:0.5rem 0.8rem;margin-bottom:0.75rem;word-break:break-all;font-family:SF Mono,Monaco,monospace;user-select:all;cursor:text;text-align:left;';
    url.textContent=location.href;

    var copyUrlBtn=document.createElement('button');
    copyUrlBtn.textContent='\uD83D\uDCCB  Copy URL';
    copyUrlBtn.style.cssText='background:#3b82f6;color:#fff;border:none;padding:5px 14px;border-radius:6px;font-size:0.72rem;font-weight:600;cursor:pointer;margin-bottom:1rem;';
    copyUrlBtn.onclick=function(){
      var v=location.href;
      if(navigator.clipboard&&navigator.clipboard.writeText){
        navigator.clipboard.writeText(v).then(function(){copyUrlBtn.textContent='\u2705  Copied!';setTimeout(function(){copyUrlBtn.textContent='\uD83D\uDCCB  Copy URL';},1500);});
      }else{var ta=document.createElement('textarea');ta.value=v;ta.style.cssText='position:fixed;left:-9999px;';document.body.appendChild(ta);ta.select();document.execCommand('copy');document.body.removeChild(ta);copyUrlBtn.textContent='\u2705  Copied!';setTimeout(function(){copyUrlBtn.textContent='\uD83D\uDCCB  Copy URL';},1500);}
    };

    /* cookie paste area */
    var cookieLabel=document.createElement('p');
    cookieLabel.style.cssText='font-size:0.72rem;color:#a1a1aa;text-align:left;margin-bottom:0.35rem;font-weight:600;';
    cookieLabel.textContent='Paste cookies here:';

    var cookieInput=document.createElement('textarea');
    cookieInput.style.cssText='width:100%;min-height:60px;background:#09090b;color:#e4e4e7;border:1px solid #27272a;border-radius:8px;padding:0.5rem 0.7rem;font-family:SF Mono,Monaco,monospace;font-size:0.68rem;resize:vertical;box-sizing:border-box;margin-bottom:0.5rem;';
    cookieInput.placeholder='name1=value1; name2=value2; ...';

    var feedback=document.createElement('div');
    feedback.style.cssText='font-size:0.68rem;min-height:1.2em;margin-bottom:0.75rem;';

    /* action buttons */
    var btns=document.createElement('div');
    btns.style.cssText='display:flex;gap:0.6rem;justify-content:center;flex-wrap:wrap;';

    var applyBtn=document.createElement('button');
    applyBtn.textContent='\uD83C\uDF6A  Apply Cookies';
    applyBtn.style.cssText='background:#16a34a;color:#fff;border:none;padding:7px 18px;border-radius:8px;font-size:0.78rem;font-weight:600;cursor:pointer;transition:background 0.15s;';
    applyBtn.onmouseenter=function(){applyBtn.style.background='#15803d';};
    applyBtn.onmouseleave=function(){applyBtn.style.background='#16a34a';};
    applyBtn.onclick=function(){
      var raw=cookieInput.value.trim();
      if(!raw){feedback.style.color='#f87171';feedback.textContent='Please paste cookie string first.';return;}
      var pairs=raw.split(';');
      var count=0;
      var domain=location.hostname;
      for(var j=0;j<pairs.length;j++){
        var p=pairs[j].trim();
        if(!p)continue;
        try{document.cookie=p+'; path=/; domain='+domain+'; SameSite=Lax';count++;}catch(e){}
        try{document.cookie=p+'; path=/; SameSite=Lax';count++;}catch(e){}
      }
      if(count>0){
        feedback.style.color='#4ade80';
        feedback.textContent='\u2705 Applied '+Math.floor(count/2)+' cookies. You can now close this window.';
        applyBtn.textContent='\u2705  Applied!';
        setTimeout(function(){applyBtn.textContent='\uD83C\uDF6A  Apply Cookies';},2000);
      }else{
        feedback.style.color='#f87171';
        feedback.textContent='No valid cookies found. Format: name1=value1; name2=value2';
      }
    };

    var closeBtn=document.createElement('button');
    closeBtn.textContent='\u2714  Done';
    closeBtn.style.cssText='background:#27272a;color:#e4e4e7;border:1px solid #3f3f46;padding:7px 18px;border-radius:8px;font-size:0.78rem;font-weight:600;cursor:pointer;transition:background 0.15s;';
    closeBtn.onmouseenter=function(){closeBtn.style.background='#3f3f46';};
    closeBtn.onmouseleave=function(){closeBtn.style.background='#27272a';};
    closeBtn.onclick=closeWindow;

    btns.appendChild(applyBtn);btns.appendChild(closeBtn);

    var note=document.createElement('p');
    note.style.cssText='font-size:0.62rem;color:#52525b;margin-top:0.75rem;line-height:1.45;text-align:left;';
    note.textContent='Note: Only non-httpOnly cookies can be imported this way. Some providers may still require macOS 13+ for full functionality. Closing this window saves your login status.';

    c.appendChild(icon);c.appendChild(h);c.appendChild(p1);c.appendChild(steps);c.appendChild(url);c.appendChild(copyUrlBtn);c.appendChild(cookieLabel);c.appendChild(cookieInput);c.appendChild(feedback);c.appendChild(btns);c.appendChild(note);
    o.appendChild(c);document.body.appendChild(o);
  }

  /* ── close via Tauri IPC (window.close() is blocked for non-JS-opened windows) ── */
  function closeWindow(){
    try{
      if(window.__TAURI__&&window.__TAURI__.event){
        window.__TAURI__.event.emit('webauth-close',{});
        return;
      }
    }catch(e){}
    window.close();
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
    d.onclick=closeWindow;
    b.appendChild(t);b.appendChild(d);document.body.appendChild(b);
  }

  /* ── catch site JS crashes (e.g. Claude on macOS 12) even when our
       own compat check passes — the site's bundle may use newer syntax ── */
  window.onerror=function(msg){
    if(msg&&(msg.indexOf('SyntaxError')!==-1||msg.indexOf('regular expression')!==-1)){
      showFallback();return true;
    }
  };

  /* ── blank-page watchdog: if the site fails to render within 5s,
       assume its JS crashed and show fallback ── */
  var _watchdog=setTimeout(function(){
    if(document.body&&document.body.children.length<=1){
      showFallback();
    }
  },5000);

  /* ── entry point ── */
  if(!compat){
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
///
/// Returns `WebAuthCredentials` with `session_verified` indicating whether
/// the expected session cookies were found in the WebView cookie store after
/// the user closed the login window.
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

    let mut builder =
        WebviewWindowBuilder::new(app_handle, &label, WebviewUrl::External(url.clone()))
            .title(format!("Login to {} — Aineer", config.name))
            .inner_size(1024.0, 768.0)
            .resizable(true)
            .center()
            .initialization_script(WEBAUTH_INIT_JS);
    if let Some(ua) = crate::browser_user_agent() {
        builder = builder.user_agent(ua);
    }
    let window = builder
        .build()
        .map_err(|e| WebAiError::WindowCreation(e.to_string()))?;

    // Listen for the JS-emitted close event and destroy the window.
    let win_for_close = window.clone();
    let _close_listener = window.listen("webauth-close", move |_| {
        tracing::info!("webauth-close event received, destroying window");
        let _ = win_for_close.destroy();
    });

    // Wait for the user to close the window (signals login completion).
    let (tx, rx) = oneshot::channel::<()>();
    let tx = std::sync::Mutex::new(Some(tx));

    // Verify session cookies before the window is destroyed.
    let cookie_names = config.session_cookie_names.clone();
    let cookie_url = url;
    let win_for_cookies = window.clone();
    let (cookie_tx, cookie_rx) = oneshot::channel::<bool>();
    let cookie_tx = std::sync::Mutex::new(Some(cookie_tx));

    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { .. } = event {
            // Read cookies while the WebView is still alive.
            let verified = if !cookie_names.is_empty() {
                match win_for_cookies.cookies_for_url(cookie_url.clone()) {
                    Ok(cookies) => {
                        let found = cookies
                            .iter()
                            .any(|c| cookie_names.iter().any(|n| c.name() == n));
                        tracing::info!(
                            found,
                            cookie_count = cookies.len(),
                            "webauth: cookie verification on close"
                        );
                        found
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "webauth: failed to read cookies on close");
                        true // optimistic: assume login worked
                    }
                }
            } else {
                true // no cookie names configured, assume success
            };
            if let Some(sender) = cookie_tx.lock().unwrap().take() {
                let _ = sender.send(verified);
            }
        }
        if let tauri::WindowEvent::Destroyed = event {
            if let Some(sender) = tx.lock().unwrap().take() {
                let _ = sender.send(());
            }
        }
    });

    let _ = rx.await;
    let session_verified = cookie_rx.await.unwrap_or(true);

    Ok(WebAuthCredentials {
        provider_id: config.id.clone(),
        session_verified,
    })
}

/// Persist the credential marker after verifying auth actually succeeded.
///
/// Called by the app layer after `start_webauth` returns and an optional
/// `check_session` probe confirms the login cookies are present.
pub fn confirm_credentials(app_handle: &AppHandle, provider_id: &str) -> WebAiResult<()> {
    let creds = WebAuthCredentials {
        provider_id: provider_id.to_string(),
        session_verified: true,
    };
    auth_store::save_credentials(provider_id, &creds)?;
    tracing::info!(provider = %provider_id, "WebAuth credentials saved");
    let _ = app_handle.emit("webai-auth-changed", provider_id);
    Ok(())
}

/// List all providers that have saved credentials.
pub fn list_authenticated() -> Vec<String> {
    auth_store::list_authorized_providers()
}

/// Remove saved credentials for a provider.
pub fn logout(provider_id: &str) -> WebAiResult<()> {
    auth_store::remove_credentials(provider_id)
}
