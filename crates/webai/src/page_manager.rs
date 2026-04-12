use std::collections::HashMap;
use std::time::Duration;

use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};

use crate::error::{WebAiError, WebAiResult};
use crate::page::WebAiPage;

/// JS injected into every WebAI page to provide a ready-check helper.
/// The Capability `webai-remote-ipc` ensures `__TAURI__` is available on
/// provider domains; this script waits for it before resolving.
const BRIDGE_INIT_JS: &str = r#"
if (!window.__webai_ready) {
  window.__webai_ready = new Promise(resolve => {
    const check = () => {
      if (window.__TAURI__ && window.__TAURI__.event) {
        resolve(true);
      } else {
        setTimeout(check, 50);
      }
    };
    check();
  });
}
"#;

const PAGE_LOAD_TIMEOUT: Duration = Duration::from_secs(30);

/// Manages a pool of hidden WebView pages, one per provider.
///
/// Each provider gets a dedicated `WebviewWindow` loaded on its domain
/// so that in-page `fetch` and DOM operations carry the right cookies.
pub struct WebAiPageManager {
    app_handle: AppHandle,
    pages: HashMap<String, WebAiPage>,
}

impl WebAiPageManager {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            pages: HashMap::new(),
        }
    }

    /// Get or lazily create the hidden WebView page for a provider.
    ///
    /// `start_url` is the provider domain page to load (e.g. `https://claude.ai/`).
    /// The window label follows the pattern `webai-{provider_id}` which is
    /// matched by the `webai-remote-ipc` Capability.
    pub async fn get_or_create(
        &mut self,
        provider_id: &str,
        start_url: &str,
    ) -> WebAiResult<&WebAiPage> {
        if !self.pages.contains_key(provider_id) {
            let page = self.create_page(provider_id, start_url).await?;
            self.pages.insert(provider_id.to_string(), page);
        }
        self.pages
            .get(provider_id)
            .ok_or_else(|| WebAiError::WindowCreation("page vanished unexpectedly".into()))
    }

    /// Close and remove a specific provider page.
    pub fn close(&mut self, provider_id: &str) {
        if let Some(page) = self.pages.remove(provider_id) {
            let _ = page.window().close();
        }
    }

    /// Close all managed pages.
    pub fn close_all(&mut self) {
        for (_, page) in self.pages.drain() {
            let _ = page.window().close();
        }
    }

    pub fn has_page(&self, provider_id: &str) -> bool {
        self.pages.contains_key(provider_id)
    }

    async fn create_page(&self, provider_id: &str, start_url: &str) -> WebAiResult<WebAiPage> {
        let label = format!("webai-{provider_id}");
        let url: url::Url = start_url
            .parse()
            .map_err(|e| WebAiError::WindowCreation(format!("invalid URL: {e}")))?;

        let window = WebviewWindowBuilder::new(&self.app_handle, &label, WebviewUrl::External(url))
            .title(format!("WebAI - {provider_id}"))
            .visible(false)
            .initialization_script(BRIDGE_INIT_JS)
            .build()
            .map_err(|e| WebAiError::WindowCreation(e.to_string()))?;

        let page = WebAiPage::new(window, provider_id.to_string());

        self.wait_for_page_ready(&page).await?;

        Ok(page)
    }

    /// Poll the page until the `__webai_ready` promise resolves (i.e. `__TAURI__` is available).
    async fn wait_for_page_ready(&self, page: &WebAiPage) -> WebAiResult<()> {
        let check_js = r#"
            const ready = await window.__webai_ready;
            return ready === true;
        "#;

        match page
            .evaluate::<bool>(check_js, Some(PAGE_LOAD_TIMEOUT))
            .await
        {
            Ok(true) => {
                tracing::debug!(provider = %page.provider_id(), "WebAI page ready");
                Ok(())
            }
            Ok(false) => Err(WebAiError::WindowCreation(
                "page ready check returned false".into(),
            )),
            Err(e) => {
                tracing::warn!(
                    provider = %page.provider_id(),
                    error = %e,
                    "WebAI page readiness check failed"
                );
                Err(e)
            }
        }
    }
}
