use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex};

use crate::error::{WebAiError, WebAiResult};
use crate::page_manager::WebAiPageManager;
use crate::provider::{ModelInfo, ProviderConfig, WebProviderClient};
use crate::providers::{
    chatgpt::ChatGptProvider, claude::ClaudeProvider, deepseek::DeepSeekProvider,
    doubao::DoubaoProvider, gemini::GeminiWebProvider, glm::GlmProvider,
    glm_intl::GlmIntlWebProvider, grok::GrokProvider, kimi::KimiProvider,
    perplexity::PerplexityWebProvider, qwen::QwenProvider, qwen_cn::QwenCnProvider,
    xiaomimo::XiaomimoProvider,
};
use crate::tool_calling::converter::{
    build_prompt_from_messages, parse_tool_response, ChatMessage, ParsedResponse, ToolChoice,
    ToolDefinition,
};
use crate::webauth;

const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 300;
const DEFAULT_PAGE_LOAD_TIMEOUT_SECS: u64 = 60;

/// High-level facade that owns the WebView page manager, provider registry,
/// and tool-calling layer.  Thread-safe and cheaply cloneable via `Arc`.
#[derive(Clone)]
pub struct WebAiEngine {
    inner: Arc<Inner>,
}

struct Inner {
    app_handle: AppHandle,
    page_manager: Mutex<WebAiPageManager>,
    providers: HashMap<String, Arc<dyn WebProviderClient>>,
    idle_timeout_secs: AtomicU64,
    page_load_timeout_secs: AtomicU64,
}

impl WebAiEngine {
    pub fn new(app_handle: AppHandle) -> Self {
        let mut p: HashMap<String, Arc<dyn WebProviderClient>> = HashMap::new();
        p.insert("chatgpt-web".into(), Arc::new(ChatGptProvider::new()));
        p.insert("claude-web".into(), Arc::new(ClaudeProvider::new()));
        p.insert("deepseek-web".into(), Arc::new(DeepSeekProvider::new()));
        p.insert("doubao-web".into(), Arc::new(DoubaoProvider::new()));
        p.insert("gemini-web".into(), Arc::new(GeminiWebProvider::new()));
        p.insert("glm-web".into(), Arc::new(GlmProvider::new()));
        p.insert("glm-intl-web".into(), Arc::new(GlmIntlWebProvider::new()));
        p.insert("grok-web".into(), Arc::new(GrokProvider::new()));
        p.insert("kimi-web".into(), Arc::new(KimiProvider::new()));
        p.insert(
            "perplexity-web".into(),
            Arc::new(PerplexityWebProvider::new()),
        );
        p.insert("qwen-web".into(), Arc::new(QwenProvider::new()));
        p.insert("qwen-cn-web".into(), Arc::new(QwenCnProvider::new()));
        p.insert("xiaomimo-web".into(), Arc::new(XiaomimoProvider::new()));

        Self {
            inner: Arc::new(Inner {
                app_handle: app_handle.clone(),
                page_manager: Mutex::new(WebAiPageManager::new(app_handle)),
                providers: p,
                idle_timeout_secs: AtomicU64::new(DEFAULT_IDLE_TIMEOUT_SECS),
                page_load_timeout_secs: AtomicU64::new(DEFAULT_PAGE_LOAD_TIMEOUT_SECS),
            }),
        }
    }

    /// List all registered WebAI providers.
    pub fn list_providers(&self) -> Vec<ProviderConfig> {
        self.inner
            .providers
            .values()
            .map(|p| p.config().clone())
            .collect()
    }

    /// List models for a specific provider (static config).
    pub fn list_models(&self, provider_id: &str) -> Vec<ModelInfo> {
        self.inner
            .providers
            .get(provider_id)
            .map(|p| p.list_models())
            .unwrap_or_default()
    }

    /// Fetch the live model list for a provider via its web API.
    ///
    /// Falls back to the static model list if the dynamic fetch fails or
    /// if no page can be created (e.g. user not authenticated).
    pub async fn fetch_models(&self, provider_name: &str) -> Vec<ModelInfo> {
        let provider_id = match self.resolve_provider_id(provider_name) {
            Some(id) => id,
            None => return vec![],
        };
        let provider = self.inner.providers[&provider_id].clone();
        let start_url = provider.config().start_url.clone();

        let mut mgr = self.inner.page_manager.lock().await;
        let load_secs = self.inner.page_load_timeout_secs.load(Ordering::Relaxed);
        mgr.set_page_load_timeout(Duration::from_secs(load_secs));

        // Only attempt dynamic fetch if a page already exists (avoid creating one
        // just for model listing — that would trigger a slow page load).
        if !mgr.has_page(&provider_id) {
            drop(mgr);
            return provider.list_models();
        }

        let page = match mgr.get_or_create(&provider_id, &start_url).await {
            Ok(p) => p.clone(),
            Err(_) => {
                drop(mgr);
                return provider.list_models();
            }
        };
        drop(mgr);

        provider
            .fetch_models(&page)
            .await
            .unwrap_or_else(|_| provider.list_models())
    }

    pub fn set_idle_timeout_secs(&self, secs: u64) {
        self.inner.idle_timeout_secs.store(secs, Ordering::Relaxed);
    }

    pub fn set_page_load_timeout_secs(&self, secs: u64) {
        self.inner
            .page_load_timeout_secs
            .store(secs, Ordering::Relaxed);
    }

    /// Resolve `webai/<provider>` or `webai/<provider>/<model>` into (provider_id, model).
    pub fn parse_webai_model(model: &str) -> Option<(&str, Option<&str>)> {
        let stripped = model.strip_prefix("webai/")?;
        if stripped.is_empty() {
            return None;
        }
        if let Some(pos) = stripped.find('/') {
            let provider = &stripped[..pos];
            let model_part = &stripped[pos + 1..];
            if model_part.is_empty() {
                Some((provider, None))
            } else {
                Some((provider, Some(model_part)))
            }
        } else {
            Some((stripped, None))
        }
    }

    /// Map a short provider name to the registered provider ID.
    fn resolve_provider_id(&self, name: &str) -> Option<String> {
        let candidate = match name {
            "chatgpt" | "openai" => "chatgpt-web",
            "claude" => "claude-web",
            "deepseek" => "deepseek-web",
            "doubao" => "doubao-web",
            "gemini" | "google" => "gemini-web",
            "glm" | "chatglm" => "glm-web",
            "glm-intl" | "glm_intl" => "glm-intl-web",
            "grok" => "grok-web",
            "kimi" | "moonshot" => "kimi-web",
            "perplexity" => "perplexity-web",
            "qwen" => "qwen-web",
            "qwen-cn" | "qwen_cn" | "qianwen" => "qwen-cn-web",
            "xiaomimo" | "mimo" => "xiaomimo-web",
            other => other,
        };
        if self.inner.providers.contains_key(candidate) {
            Some(candidate.to_string())
        } else {
            None
        }
    }

    /// Remove the credential marker for a provider and notify the UI.
    ///
    /// Called automatically when `check_session` determines that a session is no
    /// longer valid, so the settings page reflects reality instead of showing a
    /// stale "logged in" badge.
    fn invalidate_auth(&self, provider_id: &str) {
        if let Err(e) = webauth::logout(provider_id) {
            tracing::warn!(%provider_id, error = %e, "failed to remove credential marker");
        }
        let _ = self
            .inner
            .app_handle
            .emit("webai-auth-changed", provider_id);
        tracing::info!(%provider_id, "auth invalidated — credential marker removed, UI notified");
    }

    /// Fast local check: are expected session cookies present in the WebView cookie store?
    ///
    /// Returns `Some(false)` when the probe ran and no session cookies were found,
    /// `Some(true)` when at least one was found, or `None` when the probe could
    /// not run (no cookie names configured, URL parse error, cookie API error).
    async fn cookie_probe(&self, provider_id: &str, config: &ProviderConfig) -> Option<bool> {
        if config.session_cookie_names.is_empty() {
            return None;
        }
        let url = url::Url::parse(&config.start_url).ok()?;
        let mgr = self.inner.page_manager.lock().await;
        let window = mgr.get_window(provider_id)?;
        let cookies = window.cookies_for_url(url).ok()?;
        let found = cookies
            .iter()
            .any(|c| config.session_cookie_names.iter().any(|n| c.name() == n));
        Some(found)
    }

    /// Two-phase session validity check (cookie probe + JS fetch).
    ///
    /// Shared by `check_session` and `send_raw` so the logic lives in one place.
    /// Returns `true` when the session is valid, `false` when it is not.
    /// On `false`, the credential marker is automatically removed and the UI is
    /// notified.
    async fn verify_session(
        &self,
        provider_id: &str,
        provider: &Arc<dyn WebProviderClient>,
        page: &crate::page::WebAiPage,
    ) -> WebAiResult<bool> {
        let config = provider.config();

        // Phase 1: Cookie probe (fast, local).
        if let Some(false) = self.cookie_probe(provider_id, config).await {
            tracing::info!(%provider_id, "cookie probe: no session cookies — not authenticated");
            self.invalidate_auth(provider_id);
            return Ok(false);
        }

        // Phase 2: JS fetch probe (slow, network).
        let result = provider.check_session(page).await;
        if let Ok(false) = &result {
            self.invalidate_auth(provider_id);
        }
        result
    }

    /// Return the list of provider IDs that currently have an active WebView page.
    pub async fn list_active_pages(&self) -> Vec<String> {
        let mgr = self.inner.page_manager.lock().await;
        mgr.list_pages()
    }

    /// Close the WebView page for a specific provider, optionally clearing its
    /// cookies first so the session is truly terminated.
    pub async fn close_page(&self, provider_id: &str) {
        self.clear_provider_cookies(provider_id).await;
        let mut mgr = self.inner.page_manager.lock().await;
        mgr.close(provider_id);
    }

    /// Delete all cookies belonging to a provider's domain from its WebView.
    ///
    /// Must be called **before** closing/destroying the WebView window, because
    /// the cookie APIs operate on the live `WKHTTPCookieStore` / WebView2
    /// `CookieManager` / WebKitGTK `CookieManager`.
    async fn clear_provider_cookies(&self, provider_id: &str) {
        let mgr = self.inner.page_manager.lock().await;
        let window = match mgr.get_window(provider_id) {
            Some(w) => w.clone(),
            None => return,
        };
        drop(mgr);

        let provider = match self.inner.providers.get(provider_id) {
            Some(p) => p,
            None => return,
        };
        let start_url = &provider.config().start_url;
        let url = match url::Url::parse(start_url) {
            Ok(u) => u,
            Err(_) => return,
        };

        match window.cookies_for_url(url) {
            Ok(cookies) => {
                let count = cookies.len();
                for c in cookies {
                    if let Err(e) = window.delete_cookie(c) {
                        tracing::warn!(%provider_id, error = %e, "failed to delete cookie");
                    }
                }
                tracing::info!(%provider_id, count, "cleared provider cookies");
            }
            Err(e) => {
                tracing::warn!(%provider_id, error = %e, "failed to read cookies for cleanup");
            }
        }
    }

    /// Close all active WebView pages.
    pub async fn close_all_pages(&self) {
        let mut mgr = self.inner.page_manager.lock().await;
        mgr.close_all();
    }

    /// Check whether the session (cookies) for a provider is still valid.
    ///
    /// Two-phase check:
    /// 1. **Cookie probe** (fast, local) — if the provider declares
    ///    `session_cookie_names`, read the WebView cookie store via Tauri API.
    ///    If none of the expected cookies exist, return `false` immediately
    ///    without executing any JS.
    /// 2. **JS fetch probe** (slow, network) — only runs when phase 1 passes.
    ///    Each provider's `check_session` uses AbortController (8s) + evaluate
    ///    timeout (10s).
    pub async fn check_session(&self, provider_name: &str) -> WebAiResult<bool> {
        let provider_id = self
            .resolve_provider_id(provider_name)
            .ok_or_else(|| WebAiError::Provider(format!("unknown provider: {provider_name}")))?;

        let provider = self.inner.providers[&provider_id].clone();
        let start_url = provider.config().start_url.clone();

        let mut mgr = self.inner.page_manager.lock().await;
        let load_secs = self.inner.page_load_timeout_secs.load(Ordering::Relaxed);
        mgr.set_page_load_timeout(Duration::from_secs(load_secs));
        let needs_init = !mgr.has_page(&provider_id);
        let page = mgr.get_or_create(&provider_id, &start_url).await?.clone();
        drop(mgr);

        if needs_init {
            provider.init(&page).await?;
        }

        self.verify_session(&provider_id, &provider, &page).await
    }

    /// Send a plain-text message (no tool handling) and get streaming response.
    pub async fn send_raw(
        &self,
        provider_name: &str,
        model: &str,
        message: &str,
    ) -> WebAiResult<mpsc::Receiver<String>> {
        tracing::info!(%provider_name, %model, "send_raw: resolving provider");
        let provider_id = self
            .resolve_provider_id(provider_name)
            .ok_or_else(|| WebAiError::Provider(format!("unknown provider: {provider_name}")))?;

        let provider = self.inner.providers[&provider_id].clone();
        let start_url = provider.config().start_url.clone();

        tracing::info!(%provider_id, %start_url, "send_raw: acquiring page manager lock");
        let mut mgr = self.inner.page_manager.lock().await;
        let idle_secs = self.inner.idle_timeout_secs.load(Ordering::Relaxed);
        let load_secs = self.inner.page_load_timeout_secs.load(Ordering::Relaxed);
        mgr.cleanup_idle(Duration::from_secs(idle_secs));
        mgr.set_page_load_timeout(Duration::from_secs(load_secs));
        let needs_init = !mgr.has_page(&provider_id);
        tracing::info!(%provider_id, needs_init, "send_raw: get_or_create page");
        let page = mgr.get_or_create(&provider_id, &start_url).await?.clone();
        drop(mgr);

        if needs_init {
            tracing::info!(%provider_id, "send_raw: running provider init");
            provider.init(&page).await?;
        }

        tracing::info!(%provider_id, %model, "send_raw: verifying session");
        match self.verify_session(&provider_id, &provider, &page).await {
            Ok(true) => {}
            Ok(false) => {
                return Err(WebAiError::NotAuthenticated {
                    provider_id: provider_id.clone(),
                });
            }
            Err(e) => {
                tracing::warn!(%provider_id, error = %e, "session check error, proceeding anyway");
            }
        }

        tracing::info!(%provider_id, %model, "send_raw: sending message");
        provider.send_message(&page, message, model).await
    }

    /// Send an OpenAI-format request with automatic tool-calling adaptation.
    ///
    /// - If `tools` is `None`/empty: direct pass-through (chat mode)
    /// - If `tools` is present: inject tool prompt, buffer response, extract tool calls (agent mode)
    pub async fn send_openai(
        &self,
        provider_name: &str,
        model: &str,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&ToolChoice>,
    ) -> WebAiResult<OpenAiStreamResult> {
        let converted = build_prompt_from_messages(messages, tools, tool_choice);

        let rx = self
            .send_raw(provider_name, model, &converted.prompt)
            .await?;

        if !converted.has_tools {
            return Ok(OpenAiStreamResult::Streaming(rx));
        }

        let full_text = collect_stream(rx).await;
        let parsed = parse_tool_response(&full_text, tools);
        Ok(OpenAiStreamResult::Completed(parsed))
    }
}

/// Result from `send_openai`: either a live stream (chat) or a fully-parsed response (tools).
pub enum OpenAiStreamResult {
    /// Chat mode: stream text chunks directly.
    Streaming(mpsc::Receiver<String>),
    /// Agent mode: buffered and parsed for tool calls.
    Completed(ParsedResponse),
}

async fn collect_stream(mut rx: mpsc::Receiver<String>) -> String {
    let mut buf = String::new();
    while let Some(chunk) = rx.recv().await {
        buf.push_str(&chunk);
    }
    buf
}
