use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::WebAiResult;
use crate::page::WebAiPage;
use crate::provider::{ModelInfo, ProviderConfig, WebProviderClient};
use crate::sse_parser::SseLineParser;

pub struct DeepSeekProvider {
    config: ProviderConfig,
}

impl Default for DeepSeekProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl DeepSeekProvider {
    pub fn new() -> Self {
        Self {
            config: ProviderConfig {
                id: "deepseek-web".into(),
                name: "DeepSeek Web".into(),
                start_url: "https://chat.deepseek.com/".into(),
                host_key: "deepseek.com".into(),
                models: vec![
                    ModelInfo {
                        id: "deepseek-chat".into(),
                        name: "DeepSeek Chat".into(),
                        default: true,
                    },
                    ModelInfo {
                        id: "deepseek-reasoner".into(),
                        name: "DeepSeek Reasoner".into(),
                        default: false,
                    },
                ],
            },
        }
    }
}

#[async_trait]
impl WebProviderClient for DeepSeekProvider {
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
            "deepseek-chat"
        } else {
            model
        };
        let thinking = !model.contains("chat");
        let js = build_send_js(message, thinking);
        let (rx, _handle) = page.evaluate_streaming(&js, 256)?;

        let (parsed_tx, parsed_rx) = mpsc::channel::<String>(256);
        tokio::spawn(async move {
            let mut sse = SseLineParser::new();
            let mut raw_rx = rx;
            while let Some(chunk) = raw_rx.recv().await {
                sse.push(&chunk);
                for event_data in sse.drain_events() {
                    if let Some(delta) = extract_delta(&event_data) {
                        if parsed_tx.send(delta).await.is_err() {
                            return;
                        }
                    }
                }
            }
            for event_data in sse.flush() {
                if let Some(delta) = extract_delta(&event_data) {
                    let _ = parsed_tx.send(delta).await;
                }
            }
            drop(_handle);
        });
        Ok(parsed_rx)
    }

    async fn check_session(&self, page: &WebAiPage) -> WebAiResult<bool> {
        page.evaluate::<bool>(
            "const r = await fetch('https://chat.deepseek.com/api/v0/users/current', { credentials: 'include' }); return r.ok;",
            None,
        ).await
    }
}

fn build_send_js(message: &str, thinking_enabled: bool) -> String {
    let msg = serde_json::to_string(message).unwrap_or_else(|_| "\"\"".into());
    let thinking = if thinking_enabled { "true" } else { "false" };
    format!(
        r#"
const message = {msg};

const sessionRes = await fetch('https://chat.deepseek.com/api/v0/chat_session/create', {{
    method: 'POST',
    headers: {{ 'Content-Type': 'application/json' }},
    credentials: 'include',
    body: '{{}}'
}});
if (!sessionRes.ok) throw new Error('DeepSeek session create: ' + sessionRes.status);
const sessionData = await sessionRes.json();
const chatSessionId = sessionData.data?.biz_data?.id || sessionData.data?.biz_data?.chat_session_id || '';

const completionRes = await fetch('https://chat.deepseek.com/api/v0/chat/completion', {{
    method: 'POST',
    headers: {{ 'Content-Type': 'application/json' }},
    credentials: 'include',
    body: JSON.stringify({{
        chat_session_id: chatSessionId,
        parent_message_id: null,
        prompt: message,
        ref_file_ids: [],
        thinking_enabled: {thinking},
        search_enabled: true
    }})
}});

if (!completionRes.ok) {{
    const text = await completionRes.text();
    throw new Error('[DeepSeek] ' + completionRes.status + ' ' + text.slice(0, 500));
}}

const reader = completionRes.body.getReader();
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
    let choices = v.get("choices")?.as_array()?;
    let delta = choices.first()?.get("delta")?;
    let content = delta.get("content")?.as_str()?;
    if content.is_empty() {
        None
    } else {
        Some(content.to_string())
    }
}
