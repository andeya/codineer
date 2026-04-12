use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::WebAiResult;
use crate::page::WebAiPage;
use crate::provider::{ModelInfo, ProviderConfig, WebProviderClient};
use crate::sse_parser::SseLineParser;

pub struct ChatGptProvider {
    config: ProviderConfig,
}

impl Default for ChatGptProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatGptProvider {
    pub fn new() -> Self {
        Self {
            config: ProviderConfig {
                id: "chatgpt-web".into(),
                name: "ChatGPT Web".into(),
                start_url: "https://chatgpt.com/".into(),
                host_key: "chatgpt.com".into(),
                models: vec![
                    ModelInfo {
                        id: "gpt-4".into(),
                        name: "GPT-4".into(),
                        default: true,
                    },
                    ModelInfo {
                        id: "gpt-4-turbo".into(),
                        name: "GPT-4 Turbo".into(),
                        default: false,
                    },
                    ModelInfo {
                        id: "gpt-3.5-turbo".into(),
                        name: "GPT-3.5 Turbo".into(),
                        default: false,
                    },
                ],
            },
        }
    }
}

#[async_trait]
impl WebProviderClient for ChatGptProvider {
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
        let model = if model.is_empty() { "gpt-4" } else { model };
        let js = build_send_js(message, model);
        let (rx, _handle) = page.evaluate_streaming(&js, 256)?;

        let (parsed_tx, parsed_rx) = mpsc::channel::<String>(256);
        tokio::spawn(async move {
            let mut sse = SseLineParser::new();
            let mut raw_rx = rx;
            let mut prev_len = 0usize;
            while let Some(chunk) = raw_rx.recv().await {
                sse.push(&chunk);
                for event_data in sse.drain_events() {
                    if let Some(full_text) = extract_accumulated_text(&event_data) {
                        if full_text.len() > prev_len {
                            let new_part = full_text[prev_len..].to_string();
                            prev_len = full_text.len();
                            if parsed_tx.send(new_part).await.is_err() {
                                return;
                            }
                        }
                    }
                }
            }
            for event_data in sse.flush() {
                if let Some(full_text) = extract_accumulated_text(&event_data) {
                    if full_text.len() > prev_len {
                        let new_part = full_text[prev_len..].to_string();
                        let _ = parsed_tx.send(new_part).await;
                    }
                }
            }
            drop(_handle);
        });
        Ok(parsed_rx)
    }

    async fn check_session(&self, page: &WebAiPage) -> WebAiResult<bool> {
        let js = r#"
const r = await fetch('https://chatgpt.com/api/auth/session', { credentials: 'include' });
if (!r.ok) return false;
const data = await r.json();
return !!data.accessToken;
"#;
        page.evaluate::<bool>(js, None).await
    }
}

fn build_send_js(message: &str, model: &str) -> String {
    let msg = serde_json::to_string(message).unwrap_or_else(|_| "\"\"".into());
    let mdl = serde_json::to_string(model).unwrap_or_else(|_| "\"gpt-4\"".into());
    format!(
        r#"
const message = {msg};
const model = {mdl};
const msgId = crypto.randomUUID();
const parentId = crypto.randomUUID();

const body = {{
    action: 'next',
    messages: [{{ id: msgId, author: {{ role: 'user' }}, content: {{ content_type: 'text', parts: [message] }} }}],
    parent_message_id: parentId,
    model: model,
    timezone_offset_min: new Date().getTimezoneOffset(),
    history_and_training_disabled: false,
    conversation_mode: {{ kind: 'primary_assistant' }},
    force_use_sse: true,
}};

const session = await fetch('https://chatgpt.com/api/auth/session', {{ credentials: 'include' }}).then(r => r.ok ? r.json() : {{}}).catch(() => ({{}}));
const accessToken = session?.accessToken;
const deviceId = session?.oaiDeviceId || crypto.randomUUID();

const headers = {{
    'Content-Type': 'application/json',
    'Accept': 'text/event-stream',
    'oai-device-id': deviceId,
    'oai-language': 'en-US',
    ...(accessToken ? {{ Authorization: 'Bearer ' + accessToken }} : {{}}),
}};

const res = await fetch('https://chatgpt.com/backend-api/conversation', {{
    method: 'POST',
    headers,
    body: JSON.stringify(body),
    credentials: 'include',
}});

if (!res.ok) {{
    const text = await res.text();
    throw new Error('[ChatGPT] ' + res.status + ' ' + text.slice(0, 500));
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

/// ChatGPT SSE events carry the full accumulated text in `message.content.parts[-1]`.
/// We return the full text and let the caller compute the incremental diff.
fn extract_accumulated_text(json_str: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let msg = v.get("message")?;
    if msg.get("author")?.get("role")?.as_str()? != "assistant" {
        return None;
    }
    let parts = msg.get("content")?.get("parts")?.as_array()?;
    let text = parts.last()?.as_str()?;
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}
