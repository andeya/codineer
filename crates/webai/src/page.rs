use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use tauri::{Listener, WebviewWindow};
use tokio::sync::{mpsc, oneshot};

use crate::error::{WebAiError, WebAiResult};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const STREAM_EVENT: &str = "webai-stream-chunk";
const STREAM_DONE_EVENT: &str = "webai-stream-done";
const STREAM_ERROR_EVENT: &str = "webai-stream-error";
const EVAL_RESULT_EVENT: &str = "webai-eval-result";

/// Sentinel prefix injected by the error listener so provider-level parsers
/// can detect and surface JS errors that would otherwise be silently dropped
/// by format-specific parsers (SSE, JSON-lines, etc.).
const STREAM_ERROR_PREFIX: &str = "\x00__WEBAI_ERR__";

/// If `chunk` is an error injected by `evaluate_streaming`, return the
/// human-readable error text.  Provider stream loops should call this
/// before feeding chunks to their parser.
pub fn extract_stream_error(chunk: &str) -> Option<String> {
    chunk
        .strip_prefix(STREAM_ERROR_PREFIX)
        .map(|msg| format!("**Error:** {msg}"))
}

#[derive(Debug, Deserialize)]
struct EvalResultPayload {
    #[serde(rename = "requestId")]
    request_id: String,
    ok: bool,
    #[serde(default)]
    data: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamChunkPayload {
    #[serde(rename = "requestId")]
    request_id: String,
    chunk: String,
}

#[derive(Debug, Deserialize)]
struct StreamDonePayload {
    #[serde(rename = "requestId")]
    request_id: String,
}

#[derive(Debug, Deserialize)]
struct StreamErrorPayload {
    #[serde(rename = "requestId")]
    request_id: String,
    error: String,
}

/// A handle to a hidden WebView page loaded on a provider's domain.
///
/// Provides `evaluate` (request/response) and `evaluate_streaming` (chunked)
/// patterns. Communication uses `webview.eval()` to dispatch JS and
/// `__TAURI__.event.emit()` (enabled via Capability `remote.urls`) to return
/// results to the Rust side.
#[derive(Clone)]
pub struct WebAiPage {
    window: WebviewWindow,
    provider_id: String,
}

impl WebAiPage {
    pub fn new(window: WebviewWindow, provider_id: String) -> Self {
        Self {
            window,
            provider_id,
        }
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub fn window(&self) -> &WebviewWindow {
        &self.window
    }

    /// Quick session-validity check by fetching the given URL with cookies.
    ///
    /// Uses JS `AbortController` to cap the in-browser fetch at 8 seconds so a
    /// hanging network request cannot block the caller.  The Rust-side
    /// `evaluate` timeout is set to 10 seconds as a safety net.
    pub async fn check_session_via_fetch(&self, url: &str) -> WebAiResult<bool> {
        let url_js = serde_json::to_string(url).unwrap_or_else(|_| "\"\"".into());
        let js = format!(
            r#"
const c = new AbortController();
const t = setTimeout(() => c.abort(), 8000);
try {{
    const r = await fetch({url_js}, {{ credentials: 'include', signal: c.signal }});
    clearTimeout(t);
    return r.ok;
}} catch(e) {{
    clearTimeout(t);
    return false;
}}
"#
        );
        self.evaluate::<bool>(&js, Some(Duration::from_secs(10)))
            .await
    }

    /// Execute `js` in the WebView and return the deserialized result.
    ///
    /// The JS code should evaluate to a value (returned from an async IIFE).
    /// Internally the code is wrapped so the result is emitted back to Rust
    /// via a Tauri event keyed by a unique request ID.
    pub async fn evaluate<T: DeserializeOwned + Send + 'static>(
        &self,
        js: &str,
        timeout: Option<Duration>,
    ) -> WebAiResult<T> {
        let timeout = timeout.unwrap_or(DEFAULT_TIMEOUT);
        let req_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<WebAiResult<serde_json::Value>>();

        let expected_id = req_id.clone();
        let tx = Arc::new(std::sync::Mutex::new(Some(tx)));
        let tx_clone = tx.clone();
        let listener_id = self.window.listen(EVAL_RESULT_EVENT, move |event| {
            if let Ok(payload) = serde_json::from_str::<EvalResultPayload>(event.payload()) {
                if payload.request_id == expected_id {
                    let result = if payload.ok {
                        Ok(payload.data.unwrap_or(serde_json::Value::Null))
                    } else {
                        Err(WebAiError::JsError(
                            payload.error.unwrap_or_else(|| "unknown JS error".into()),
                        ))
                    };
                    if let Some(sender) = tx_clone.lock().unwrap().take() {
                        let _ = sender.send(result);
                    }
                }
            }
        });

        let wrapped = Self::wrap_eval_js(&req_id, js, timeout);
        if let Err(e) = self.window.eval(&wrapped) {
            self.window.unlisten(listener_id);
            return Err(WebAiError::Eval(e.to_string()));
        }

        let result = tokio::time::timeout(timeout, rx).await;
        self.window.unlisten(listener_id);

        match result {
            Ok(Ok(Ok(value))) => serde_json::from_value(value).map_err(WebAiError::Deserialize),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(_)) => Err(WebAiError::ChannelClosed),
            Err(_) => Err(WebAiError::Timeout(timeout)),
        }
    }

    /// Execute `js` that streams data back chunk-by-chunk.
    ///
    /// The JS code must emit chunks using the global `__webai_stream(requestId, chunk)`
    /// helper and signal completion with `__webai_stream_done(requestId)`.
    /// Returns an `mpsc::Receiver` that yields chunks as they arrive.
    pub fn evaluate_streaming(
        &self,
        js: &str,
        buffer_size: usize,
    ) -> WebAiResult<(mpsc::Receiver<String>, StreamHandle)> {
        let req_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::channel::<String>(buffer_size);

        let tx_shared = Arc::new(std::sync::Mutex::new(Some(tx)));

        let expected_id = req_id.clone();
        let tx_chunk = tx_shared.clone();
        let chunk_listener = self.window.listen(STREAM_EVENT, move |event| {
            if let Ok(payload) = serde_json::from_str::<StreamChunkPayload>(event.payload()) {
                if payload.request_id == expected_id {
                    if let Some(ref sender) = *tx_chunk.lock().unwrap() {
                        let _ = sender.try_send(payload.chunk);
                    }
                }
            }
        });

        let expected_id = req_id.clone();
        let tx_err = tx_shared.clone();
        let error_listener = self.window.listen(STREAM_ERROR_EVENT, move |event| {
            if let Ok(payload) = serde_json::from_str::<StreamErrorPayload>(event.payload()) {
                if payload.request_id == expected_id {
                    if let Some(ref sender) = *tx_err.lock().unwrap() {
                        let msg = format!("{}{}", STREAM_ERROR_PREFIX, payload.error);
                        let _ = sender.try_send(msg);
                    }
                }
            }
        });

        let expected_id = req_id.clone();
        let tx_done = tx_shared;
        let done_listener = self.window.listen(STREAM_DONE_EVENT, move |event| {
            if let Ok(payload) = serde_json::from_str::<StreamDonePayload>(event.payload()) {
                if payload.request_id == expected_id {
                    let _ = tx_done.lock().unwrap().take();
                }
            }
        });

        let wrapped = Self::wrap_streaming_js(&req_id, js);
        if let Err(e) = self.window.eval(&wrapped) {
            self.window.unlisten(chunk_listener);
            self.window.unlisten(error_listener);
            self.window.unlisten(done_listener);
            return Err(WebAiError::Eval(e.to_string()));
        }

        let handle = StreamHandle {
            window: self.window.clone(),
            chunk_listener,
            error_listener,
            done_listener,
        };
        Ok((rx, handle))
    }

    /// Navigate the WebView to a new URL (e.g. switching conversation endpoints).
    pub fn navigate(&self, url: &str) -> WebAiResult<()> {
        let js = format!(
            "window.location.href = {}",
            serde_json::to_string(url).unwrap_or_default()
        );
        self.window
            .eval(&js)
            .map_err(|e| WebAiError::Eval(e.to_string()))
    }

    /// Wrap user JS so the result is emitted back via Tauri event.
    ///
    /// Includes a JS-level timeout (`__JS_TIMEOUT_MS`) that races against the
    /// user code.  If the user's `await fetch(...)` or any other promise hangs
    /// indefinitely, the timeout fires first and emits an error back to Rust,
    /// preventing the `evaluate` call from waiting until the Rust-side timeout
    /// (120s) expires silently.
    fn wrap_eval_js(req_id: &str, js: &str, timeout: Duration) -> String {
        let rust_ms = timeout.as_millis() as u64;
        let js_timeout_ms = rust_ms.saturating_sub(2000).max(3000).min(rust_ms);
        format!(
            r#"(async () => {{
  const __rid = '{req_id}';
  const __emit = window.__TAURI__.event.emit.bind(window.__TAURI__.event);
  let __done = false;
  const __finish = async (ok, data, error) => {{
    if (__done) return;
    __done = true;
    await __emit('{EVAL_RESULT_EVENT}', {{ requestId: __rid, ok, data, error }});
  }};
  const __timer = setTimeout(() => {{
    __finish(false, null, 'JS execution timed out ({js_timeout_ms}ms)');
  }}, {js_timeout_ms});
  try {{
    const __r = await (async () => {{ {js} }})();
    clearTimeout(__timer);
    await __finish(true, __r, null);
  }} catch(__e) {{
    clearTimeout(__timer);
    await __finish(false, null, __e.message || String(__e));
  }}
}})();"#
        )
    }

    /// Wrap streaming JS — injects `__webai_stream` and `__webai_stream_done` helpers.
    ///
    /// On JS error the message is sent via a dedicated error event so it
    /// bypasses provider-specific parsers (e.g. SSE) and reaches the caller.
    fn wrap_streaming_js(req_id: &str, js: &str) -> String {
        format!(
            r#"(async () => {{
  const __reqId = '{req_id}';
  const __webai_stream = async (chunk) => {{
    await window.__TAURI__.event.emit('{STREAM_EVENT}',
      {{ requestId: __reqId, chunk: chunk }});
  }};
  const __webai_stream_done = async () => {{
    await window.__TAURI__.event.emit('{STREAM_DONE_EVENT}',
      {{ requestId: __reqId }});
  }};
  try {{
    await (async () => {{ {js} }})();
    await __webai_stream_done();
  }} catch(__e) {{
    await window.__TAURI__.event.emit('{STREAM_ERROR_EVENT}',
      {{ requestId: __reqId, error: __e.message || String(__e) }});
    await __webai_stream_done();
  }}
}})();"#
        )
    }
}

/// RAII guard that unlistens stream events when dropped.
pub struct StreamHandle {
    window: WebviewWindow,
    chunk_listener: tauri::EventId,
    error_listener: tauri::EventId,
    done_listener: tauri::EventId,
}

impl Drop for StreamHandle {
    fn drop(&mut self) {
        self.window.unlisten(self.chunk_listener);
        self.window.unlisten(self.error_listener);
        self.window.unlisten(self.done_listener);
    }
}
