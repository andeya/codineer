use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::WebAiResult;
use crate::page::WebAiPage;
use crate::provider::{ModelInfo, ProviderConfig, WebProviderClient};
use crate::sse_parser::SseLineParser;

pub struct QwenCnProvider {
    config: ProviderConfig,
}

impl Default for QwenCnProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl QwenCnProvider {
    pub fn new() -> Self {
        Self {
            config: ProviderConfig {
                id: "qwen-cn-web".into(),
                name: "Qwen CN Web".into(),
                start_url: "https://www.qianwen.com/".into(),
                host_key: "qianwen.com".into(),
                models: vec![
                    ModelInfo {
                        id: "Qwen3.5-Plus".into(),
                        name: "Qwen 3.5 Plus (CN)".into(),
                        default: true,
                    },
                    ModelInfo {
                        id: "Qwen3.5-Turbo".into(),
                        name: "Qwen 3.5 Turbo (CN)".into(),
                        default: false,
                    },
                ],
            },
        }
    }
}

#[async_trait]
impl WebProviderClient for QwenCnProvider {
    fn provider_id(&self) -> &str {
        &self.config.id
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    async fn init(&self, _page: &WebAiPage) -> WebAiResult<()> {
        Ok(())
    }

    async fn send_message(
        &self,
        page: &WebAiPage,
        message: &str,
        model: &str,
    ) -> WebAiResult<mpsc::Receiver<String>> {
        let model = if model.is_empty() {
            "Qwen3.5-Plus"
        } else {
            model
        };
        let js = build_send_js(message, model);
        let (rx, _handle) = page.evaluate_streaming(&js, 256)?;

        let (tx, parsed_rx) = mpsc::channel::<String>(256);
        tokio::spawn(async move {
            let mut sse = SseLineParser::new();
            let mut raw_rx = rx;
            while let Some(chunk) = raw_rx.recv().await {
                sse.push(&chunk);
                for ev in sse.drain_events() {
                    if let Some(d) = extract_delta(&ev) {
                        if tx.send(d).await.is_err() {
                            return;
                        }
                    }
                }
            }
            for ev in sse.flush() {
                if let Some(d) = extract_delta(&ev) {
                    let _ = tx.send(d).await;
                }
            }
            drop(_handle);
        });
        Ok(parsed_rx)
    }

    async fn check_session(&self, page: &WebAiPage) -> WebAiResult<bool> {
        page.evaluate::<bool>(
            "const r = await fetch('https://www.qianwen.com/', { credentials: 'include' }); return r.ok;",
            None,
        )
        .await
    }
}

fn build_send_js(message: &str, model: &str) -> String {
    let msg = serde_json::to_string(message).unwrap_or_else(|_| "\"\"".into());
    let mdl = serde_json::to_string(model).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"
function __qwenCnCookie(name) {{
  const parts = document.cookie.split(';');
  for (const p of parts) {{
    const idx = p.indexOf('=');
    if (idx < 0) continue;
    const k = p.slice(0, idx).trim();
    if (k.toLowerCase() === name.toLowerCase()) return decodeURIComponent(p.slice(idx + 1).trim());
  }}
  return '';
}}
const xsrf = __qwenCnCookie('XSRF-TOKEN') || __qwenCnCookie('x-xsrf-token') || __qwenCnCookie('csrf_token') || '';
const sessionId = crypto.randomUUID();
const qs = new URLSearchParams();
qs.set('stream', 'true');
qs.set('_t', String(Date.now()));
const url = 'https://chat2.qianwen.com/api/v2/chat?' + qs.toString();
const res = await fetch(url, {{
  method: 'POST',
  headers: {{
    'Content-Type': 'application/json',
    'Accept': 'text/event-stream',
    'x-xsrf-token': xsrf,
    'x-platform': 'pc_tongyi'
  }},
  credentials: 'include',
  body: JSON.stringify({{
    model: {mdl},
    session_id: sessionId,
    messages: [{{ content: {msg}, mime_type: 'text/plain' }}],
    protocol_version: 'v2',
    biz_id: 'ai_qwen'
  }})
}});
if (!res.ok) {{
  const t = await res.text();
  throw new Error('[Qwen CN] ' + res.status + ' ' + t.slice(0, 500));
}}
const reader = res.body.getReader();
const decoder = new TextDecoder();
while (true) {{
  const {{ done, value }} = await reader.read();
  if (done) break;
  await __webai_stream(decoder.decode(value, {{ stream: true }}));
}}
"#
    )
}

fn extract_delta(json_str: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;
    if let Some(choices) = v.get("choices").and_then(|c| c.as_array()) {
        if let Some(delta) = choices.first().and_then(|c| c.get("delta")) {
            if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                if !content.is_empty() {
                    return Some(content.to_string());
                }
            }
        }
    }
    if let Some(t) = v.get("text").and_then(|t| t.as_str()) {
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    None
}
