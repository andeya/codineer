use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::WebAiResult;
use crate::page::WebAiPage;
use crate::provider::{ModelInfo, ProviderConfig, WebProviderClient};
use crate::sse_parser::SseLineParser;

pub struct DoubaoProvider {
    config: ProviderConfig,
}

impl Default for DoubaoProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl DoubaoProvider {
    pub fn new() -> Self {
        Self {
            config: ProviderConfig {
                id: "doubao-web".into(),
                name: "Doubao Web".into(),
                start_url: "https://www.doubao.com/chat/".into(),
                host_key: "doubao.com".into(),
                models: vec![
                    ModelInfo {
                        id: "doubao-seed-2.0".into(),
                        name: "Doubao Seed 2.0 (Web)".into(),
                        default: true,
                    },
                    ModelInfo {
                        id: "doubao-pro".into(),
                        name: "Doubao Pro (Web)".into(),
                        default: false,
                    },
                ],
            },
        }
    }

    fn default_model(&self) -> &str {
        self.config
            .models
            .iter()
            .find(|m| m.default)
            .map(|m| m.id.as_str())
            .unwrap_or("doubao-seed-2.0")
    }
}

#[async_trait]
impl WebProviderClient for DoubaoProvider {
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
            self.default_model()
        } else {
            model
        };
        let js = build_send_js(message, model);
        let (rx, _handle) = page.evaluate_streaming(&js, 256)?;

        let (parsed_tx, parsed_rx) = mpsc::channel::<String>(256);
        tokio::spawn(async move {
            let mut sse = SseLineParser::new();
            let mut raw_rx = rx;
            while let Some(chunk) = raw_rx.recv().await {
                sse.push(&chunk);
                for event_data in sse.drain_events() {
                    if let Some(delta) = extract_doubao_delta(&event_data) {
                        if parsed_tx.send(delta).await.is_err() {
                            return;
                        }
                    }
                }
            }
            for event_data in sse.flush() {
                if let Some(delta) = extract_doubao_delta(&event_data) {
                    let _ = parsed_tx.send(delta).await;
                }
            }
            drop(_handle);
        });

        Ok(parsed_rx)
    }

    async fn check_session(&self, page: &WebAiPage) -> WebAiResult<bool> {
        page.evaluate::<bool>(
            "const r = await fetch('https://www.doubao.com/chat/', { credentials: 'include', method: 'GET' }); return r.ok;",
            None,
        )
        .await
    }
}

fn build_send_js(message: &str, model: &str) -> String {
    let message_escaped = serde_json::to_string(message).unwrap_or_else(|_| "\"\"".into());
    let model_escaped = serde_json::to_string(model).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"
const message = {message_escaped};
const model = {model_escaped};

const body = {{
  messages: [{{
    role: 'user',
    content_type: 2001,
    content: JSON.stringify({{ text: message }})
  }}],
  completion_option: {{
    need_create_conversation: true
  }},
  model: model
}};

const completionRes = await fetch('https://www.doubao.com/samantha/chat/completion', {{
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
  throw new Error('[Doubao] ' + completionRes.status + ' ' + text.slice(0, 500));
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

fn extract_doubao_delta(json_str: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;
    if let Some(text) = v.get("text").and_then(|x| x.as_str()) {
        if !text.is_empty() {
            return Some(text.to_string());
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
    fn extract_choices_delta_content() {
        let j = r#"{"choices":[{"delta":{"content":"hi"}}]}"#;
        assert_eq!(extract_doubao_delta(j), Some("hi".into()));
    }

    #[test]
    fn extract_top_level_text() {
        let j = r#"{"text":"hello"}"#;
        assert_eq!(extract_doubao_delta(j), Some("hello".into()));
    }
}
