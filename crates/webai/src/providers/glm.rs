use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::WebAiResult;
use crate::page::WebAiPage;
use crate::provider::{ModelInfo, ProviderConfig, WebProviderClient};
use crate::sse_parser::SseLineParser;

pub struct GlmProvider {
    config: ProviderConfig,
}

impl Default for GlmProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GlmProvider {
    pub fn new() -> Self {
        Self {
            config: ProviderConfig {
                id: "glm-web".into(),
                name: "ChatGLM (Web)".into(),
                start_url: "https://chatglm.cn".into(),
                host_key: "chatglm.cn".into(),
                models: vec![ModelInfo {
                    id: "glm-4-plus".into(),
                    name: "GLM-4 Plus".into(),
                    default: true,
                }],
            },
        }
    }

    fn default_model(&self) -> &str {
        self.config
            .models
            .iter()
            .find(|m| m.default)
            .map(|m| m.id.as_str())
            .unwrap_or("glm-4-plus")
    }
}

#[async_trait]
impl WebProviderClient for GlmProvider {
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
                    if let Some(delta) = extract_glm_delta(&event_data) {
                        if parsed_tx.send(delta).await.is_err() {
                            return;
                        }
                    }
                }
            }
            for event_data in sse.flush() {
                if let Some(delta) = extract_glm_delta(&event_data) {
                    let _ = parsed_tx.send(delta).await;
                }
            }
            drop(_handle);
        });

        Ok(parsed_rx)
    }

    async fn check_session(&self, page: &WebAiPage) -> WebAiResult<bool> {
        page.evaluate::<bool>(
            "const r = await fetch('https://chatglm.cn/', { credentials: 'include', method: 'GET' }); return r.ok;",
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

const completionRes = await fetch('https://chatglm.cn/chatglm/backend-api/assistant/stream', {{
  method: 'POST',
  headers: {{
    'Content-Type': 'application/json',
    'Accept': 'text/event-stream',
    'App-Name': 'chatglm'
  }},
  credentials: 'include',
  body: JSON.stringify({{
    assistant_id: '65940acff94777010aa6b796',
    conversation_id: '',
    model: model,
    messages: [{{
      role: 'user',
      content: [{{ type: 'text', text: message }}]
    }}]
  }})
}});

if (!completionRes.ok) {{
  const text = await completionRes.text();
  throw new Error('[GLM] ' + completionRes.status + ' ' + text.slice(0, 500));
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

fn extract_glm_delta(json_str: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;
    if let Some(data) = v.get("data").and_then(|x| x.as_str()) {
        if !data.is_empty() {
            return Some(data.to_string());
        }
    }
    let parts = v.get("parts").and_then(|p| p.as_array())?;
    let p0 = parts.first()?;
    let content_arr = p0.get("content").and_then(|c| c.as_array())?;
    let c0 = content_arr.first()?;
    let text = c0.get("text").and_then(|t| t.as_str())?;
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_parts_text() {
        let j = r#"{"parts":[{"content":[{"type":"text","text":"x"}]}]}"#;
        assert_eq!(extract_glm_delta(j), Some("x".into()));
    }

    #[test]
    fn extract_data_string() {
        let j = r#"{"data":"chunk"}"#;
        assert_eq!(extract_glm_delta(j), Some("chunk".into()));
    }
}
