use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::WebAiResult;
use crate::page::WebAiPage;
use crate::provider::{ModelInfo, ProviderConfig, WebProviderClient};
use crate::sse_parser::SseLineParser;

pub struct XiaomimoProvider {
    config: ProviderConfig,
}

impl Default for XiaomimoProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl XiaomimoProvider {
    pub fn new() -> Self {
        Self {
            config: ProviderConfig {
                id: "xiaomimo-web".into(),
                name: "Xiaomi MiMo Web".into(),
                start_url: "https://aistudio.xiaomimimo.com".into(),
                host_key: "xiaomimimo.com".into(),
                models: vec![ModelInfo {
                    id: "xiaomimo-chat".into(),
                    name: "MiMo Chat".into(),
                    default: true,
                }],
            },
        }
    }
}

#[async_trait]
impl WebProviderClient for XiaomimoProvider {
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
            let mut sse = SseLineParser::new();
            let mut raw_rx = rx;
            while let Some(chunk) = raw_rx.recv().await {
                sse.push(&chunk);
                for event_data in sse.drain_events() {
                    if let Some(delta) = extract_xiaomimo_delta(&event_data) {
                        if parsed_tx.send(delta).await.is_err() {
                            return;
                        }
                    }
                }
            }
            for event_data in sse.flush() {
                if let Some(delta) = extract_xiaomimo_delta(&event_data) {
                    let _ = parsed_tx.send(delta).await;
                }
            }
            drop(_handle);
        });

        Ok(parsed_rx)
    }

    async fn check_session(&self, page: &WebAiPage) -> WebAiResult<bool> {
        page.evaluate::<bool>(
            "const r = await fetch('https://aistudio.xiaomimimo.com/', { credentials: 'include', method: 'GET' }); return r.ok;",
            None,
        )
        .await
    }
}

fn build_send_js(message: &str) -> String {
    let message_escaped = serde_json::to_string(message).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"
const message = {message_escaped};
const msgId = (typeof crypto !== 'undefined' && crypto.randomUUID)
  ? crypto.randomUUID()
  : String(Date.now()) + '-' + Math.random().toString(36).slice(2);

const body = {{
  msgId: msgId,
  conversationId: '',
  query: message,
  modelConfig: {{
    enableThinking: false,
    temperature: 0.8,
    topP: 0.95,
    webSearchStatus: 'disabled',
    model: 'mimo-v2-flash-studio'
  }},
  multiMedias: []
}};

const completionRes = await fetch('https://aistudio.xiaomimimo.com/open-apis/bot/chat', {{
  method: 'POST',
  headers: {{
    'Content-Type': 'application/json',
    'Accept': 'text/event-stream'
  }},
  credentials: 'include',
  body: JSON.stringify(body)
}});

if (!completionRes.ok) {{
  const text = await completionRes.text();
  throw new Error('[MiMo] ' + completionRes.status + ' ' + text.slice(0, 500));
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

fn extract_xiaomimo_delta(json_str: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;
    if let Some(text) = v.get("text").and_then(|x| x.as_str()) {
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    if let Some(data) = v.get("data").and_then(|x| x.as_str()) {
        if !data.is_empty() {
            return Some(data.to_string());
        }
    }
    let choice = v
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())?;
    let delta = choice.get("delta")?;
    if let Some(content) = delta.get("content").and_then(|x| x.as_str()) {
        if !content.is_empty() {
            return Some(content.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_delta_content() {
        let j = r#"{"choices":[{"delta":{"content":"a"}}]}"#;
        assert_eq!(extract_xiaomimo_delta(j), Some("a".into()));
    }

    #[test]
    fn extract_text_field() {
        let j = r#"{"text":"t"}"#;
        assert_eq!(extract_xiaomimo_delta(j), Some("t".into()));
    }

    #[test]
    fn extract_data_field() {
        let j = r#"{"data":"d"}"#;
        assert_eq!(extract_xiaomimo_delta(j), Some("d".into()));
    }
}
