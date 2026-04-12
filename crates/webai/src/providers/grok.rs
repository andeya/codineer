use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::WebAiResult;
use crate::page::WebAiPage;
use crate::provider::{ModelInfo, ProviderConfig, WebProviderClient};

pub struct GrokProvider {
    config: ProviderConfig,
}

impl Default for GrokProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GrokProvider {
    pub fn new() -> Self {
        Self {
            config: ProviderConfig {
                id: "grok-web".into(),
                name: "Grok Web".into(),
                start_url: "https://grok.com".into(),
                host_key: "grok.com".into(),
                models: vec![
                    ModelInfo {
                        id: "grok-2".into(),
                        name: "Grok 2 (Web)".into(),
                        default: true,
                    },
                    ModelInfo {
                        id: "grok-1".into(),
                        name: "Grok 1 (Web)".into(),
                        default: false,
                    },
                ],
            },
        }
    }
}

#[async_trait]
impl WebProviderClient for GrokProvider {
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
        _model: &str,
    ) -> WebAiResult<mpsc::Receiver<String>> {
        let js = build_send_js(message);
        let (rx, _handle) = page.evaluate_streaming(&js, 256)?;

        let (parsed_tx, parsed_rx) = mpsc::channel::<String>(256);
        tokio::spawn(async move {
            let mut buf = String::new();
            let mut raw_rx = rx;
            while let Some(chunk) = raw_rx.recv().await {
                buf.push_str(&chunk);
                let mut consumed = 0;
                while let Some(rel) = buf[consumed..].find('\n') {
                    let line = buf[consumed..consumed + rel].trim();
                    consumed += rel + 1;
                    if line.is_empty() {
                        continue;
                    }
                    if let Some(delta) = extract_delta(line) {
                        if parsed_tx.send(delta).await.is_err() {
                            return;
                        }
                    }
                }
                if consumed > 0 {
                    buf.drain(..consumed);
                }
            }
            if !buf.trim().is_empty() {
                if let Some(delta) = extract_delta(buf.trim()) {
                    let _ = parsed_tx.send(delta).await;
                }
            }
            drop(_handle);
        });
        Ok(parsed_rx)
    }

    async fn check_session(&self, page: &WebAiPage) -> WebAiResult<bool> {
        page.evaluate::<bool>(
            "const r = await fetch('https://grok.com/rest/app-chat/conversations?limit=1', { credentials: 'include' }); return r.ok;",
            None,
        ).await
    }
}

fn build_send_js(message: &str) -> String {
    let msg = serde_json::to_string(message).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"
const message = {msg};

let convId = null;
const listRes = await fetch('https://grok.com/rest/app-chat/conversations?limit=1', {{ credentials: 'include' }});
if (listRes.ok) {{
    const list = await listRes.json();
    convId = list?.conversations?.[0]?.conversationId;
}}
if (!convId) {{
    const createRes = await fetch('https://grok.com/rest/app-chat/conversations', {{
        method: 'POST',
        headers: {{ 'Content-Type': 'application/json' }},
        credentials: 'include',
        body: '{{}}'
    }});
    if (createRes.ok) {{
        const d = await createRes.json();
        convId = d?.conversationId || d?.id;
    }}
}}
if (!convId) throw new Error('Grok: could not get/create conversation');

const body = {{
    message: message,
    parentResponseId: crypto.randomUUID(),
    disableSearch: false,
    enableImageGeneration: false,
    returnImageBytes: false,
    fileAttachments: [],
    enableImageStreaming: false,
    forceConcise: false,
    toolOverrides: {{}},
    sendFinalMetadata: true,
    isReasoning: false,
    disableTextFollowUps: false,
    isAsyncChat: false,
}};

const res = await fetch('https://grok.com/rest/app-chat/conversations/' + convId + '/responses', {{
    method: 'POST',
    headers: {{ 'Content-Type': 'application/json' }},
    credentials: 'include',
    body: JSON.stringify(body),
}});
if (!res.ok) {{
    const text = await res.text();
    throw new Error('[Grok] ' + res.status + ' ' + text.slice(0, 500));
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

/// Grok uses NDJSON (one JSON object per line) with `contentDelta`.
fn extract_delta(json_str: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;
    for key in &["contentDelta", "textDelta", "content", "text", "delta"] {
        if let Some(s) = v.get(key).and_then(|v| v.as_str()) {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}
