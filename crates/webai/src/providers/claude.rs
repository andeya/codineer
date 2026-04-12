use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::error::{WebAiError, WebAiResult};
use crate::page::WebAiPage;
use crate::provider::{ModelInfo, ProviderConfig, WebProviderClient};
use crate::sse_parser::SseLineParser;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeAuth {
    #[serde(default)]
    pub session_key: Option<String>,
    #[serde(default)]
    pub cookie: Option<String>,
    #[serde(default)]
    pub organization_id: Option<String>,
}

pub struct ClaudeProvider {
    config: ProviderConfig,
    organization_id: std::sync::Mutex<Option<String>>,
}

impl Default for ClaudeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeProvider {
    pub fn new() -> Self {
        Self {
            config: ProviderConfig {
                id: "claude-web".into(),
                name: "Claude Web".into(),
                start_url: "https://claude.ai/".into(),
                host_key: "claude.ai".into(),
                models: vec![
                    ModelInfo {
                        id: "claude-sonnet-4-20250514".into(),
                        name: "Claude Sonnet 4".into(),
                        default: true,
                    },
                    ModelInfo {
                        id: "claude-sonnet-4-6".into(),
                        name: "Claude Sonnet 4.6".into(),
                        default: false,
                    },
                    ModelInfo {
                        id: "claude-opus-4-20250514".into(),
                        name: "Claude Opus 4".into(),
                        default: false,
                    },
                    ModelInfo {
                        id: "claude-opus-4-6".into(),
                        name: "Claude Opus 4.6".into(),
                        default: false,
                    },
                    ModelInfo {
                        id: "claude-haiku-4-20250514".into(),
                        name: "Claude Haiku 4".into(),
                        default: false,
                    },
                    ModelInfo {
                        id: "claude-haiku-4-6".into(),
                        name: "Claude Haiku 4.6".into(),
                        default: false,
                    },
                ],
            },
            organization_id: std::sync::Mutex::new(None),
        }
    }

    fn default_model(&self) -> &str {
        self.config
            .models
            .iter()
            .find(|m| m.default)
            .map(|m| m.id.as_str())
            .unwrap_or("claude-sonnet-4-20250514")
    }
}

#[async_trait]
impl WebProviderClient for ClaudeProvider {
    fn provider_id(&self) -> &str {
        &self.config.id
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    async fn init(&self, page: &WebAiPage) -> WebAiResult<()> {
        if self.organization_id.lock().unwrap().is_some() {
            return Ok(());
        }

        let org_id: Option<String> = page.evaluate(JS_DISCOVER_ORG, None).await.unwrap_or(None);

        if let Some(ref id) = org_id {
            tracing::info!(org_id = %id, "Claude: discovered organization");
        }
        *self.organization_id.lock().unwrap() = org_id;
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

        let org_id = self.organization_id.lock().unwrap().clone();
        let conv_uuid = uuid::Uuid::new_v4().to_string();

        let js_create_and_stream = build_send_js(&org_id, &conv_uuid, model, message);

        let (rx, _handle) = page.evaluate_streaming(&js_create_and_stream, 256)?;

        let (parsed_tx, parsed_rx) = mpsc::channel::<String>(256);
        tokio::spawn(async move {
            let mut sse = SseLineParser::new();
            let mut raw_rx = rx;
            while let Some(chunk) = raw_rx.recv().await {
                sse.push(&chunk);
                for event_data in sse.drain_events() {
                    if let Some(delta) = extract_claude_delta(&event_data) {
                        if parsed_tx.send(delta).await.is_err() {
                            return;
                        }
                    }
                }
            }
            for event_data in sse.flush() {
                if let Some(delta) = extract_claude_delta(&event_data) {
                    let _ = parsed_tx.send(delta).await;
                }
            }
            // _handle is moved into this task so listeners stay alive
            drop(_handle);
        });

        Ok(parsed_rx)
    }

    async fn check_session(&self, page: &WebAiPage) -> WebAiResult<bool> {
        let result: CheckResult = page
            .evaluate(JS_CHECK_SESSION, None)
            .await
            .map_err(|e| WebAiError::Provider(format!("session check failed: {e}")))?;
        Ok(result.ok)
    }
}

#[derive(Deserialize)]
struct CheckResult {
    ok: bool,
}

// ---------------------------------------------------------------------------
// JS code constants
// ---------------------------------------------------------------------------

const JS_DISCOVER_ORG: &str = r#"
const res = await fetch('https://claude.ai/api/organizations', { credentials: 'include' });
if (!res.ok) return null;
const orgs = await res.json();
return orgs[0]?.uuid ?? null;
"#;

const JS_CHECK_SESSION: &str = r#"
const res = await fetch('https://claude.ai/api/organizations', { credentials: 'include' });
return { ok: res.ok, status: res.status };
"#;

fn build_send_js(org_id: &Option<String>, conv_uuid: &str, model: &str, message: &str) -> String {
    let api_base = "https://claude.ai/api";
    let org_path = match org_id {
        Some(id) => format!("/organizations/{id}"),
        None => String::new(),
    };
    let message_escaped = serde_json::to_string(message).unwrap_or_else(|_| "\"\"".into());
    let model_escaped = serde_json::to_string(model).unwrap_or_else(|_| "\"\"".into());
    let conv_uuid_escaped = serde_json::to_string(conv_uuid).unwrap_or_else(|_| "\"\"".into());
    let tz = "Etc/UTC";

    format!(
        r#"
const apiBase = '{api_base}';
const orgPath = '{org_path}';
const convUuid = {conv_uuid_escaped};
const model = {model_escaped};
const message = {message_escaped};

const createUrl = apiBase + orgPath + '/chat_conversations';
const createRes = await fetch(createUrl, {{
  method: 'POST',
  headers: {{ 'Content-Type': 'application/json' }},
  credentials: 'include',
  body: JSON.stringify({{ name: '', uuid: convUuid }})
}});

if (!createRes.ok) {{
  const text = await createRes.text();
  throw new Error('[create_conversation] ' + createRes.status + ' ' + text.slice(0, 500));
}}

const conv = await createRes.json();
const completionUrl = apiBase + orgPath + '/chat_conversations/' + conv.uuid + '/completion';
const completionRes = await fetch(completionUrl, {{
  method: 'POST',
  headers: {{ 'Content-Type': 'application/json', 'Accept': 'text/event-stream' }},
  credentials: 'include',
  body: JSON.stringify({{
    prompt: message,
    parent_message_uuid: '00000000-0000-4000-8000-000000000000',
    model: model,
    timezone: '{tz}',
    rendering_mode: 'messages',
    attachments: [],
    files: [],
    locale: 'en-US',
    personalized_styles: [],
    sync_sources: [],
    tools: []
  }})
}});

if (!completionRes.ok) {{
  const text = await completionRes.text();
  throw new Error('[completion] ' + completionRes.status + ' ' + text.slice(0, 500));
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

/// Extract text delta from a parsed Claude SSE JSON event.
fn extract_claude_delta(json_str: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let obj = value.as_object()?;

    let event_type = obj.get("type")?.as_str()?;

    if event_type == "content_block_delta" {
        let delta = obj.get("delta")?;
        if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }

    if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    if let Some(content) = obj.get("content").and_then(|v| v.as_str()) {
        if !content.is_empty() {
            return Some(content.to_string());
        }
    }
    if let Some(delta) = obj.get("delta").and_then(|v| v.as_str()) {
        if !delta.is_empty() {
            return Some(delta.to_string());
        }
    }
    if let Some(choice) = obj
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
    {
        if let Some(content) = choice
            .get("delta")
            .and_then(|d| d.get("content"))
            .and_then(|v| v.as_str())
        {
            if !content.is_empty() {
                return Some(content.to_string());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_content_block_delta() {
        let json = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        assert_eq!(extract_claude_delta(json), Some("Hello".to_string()));
    }

    #[test]
    fn test_extract_thinking_ignored() {
        let json = r#"{"type":"content_block_start","content_block":{"type":"thinking"}}"#;
        assert_eq!(extract_claude_delta(json), None);
    }

    #[test]
    fn test_extract_generic_text() {
        let json = r#"{"type":"unknown","text":"fallback"}"#;
        assert_eq!(extract_claude_delta(json), Some("fallback".to_string()));
    }

    #[test]
    fn test_build_send_js_includes_message() {
        let js = build_send_js(
            &Some("org-123".into()),
            "conv-456",
            "claude-sonnet-4-6",
            "Hello Claude",
        );
        assert!(js.contains("/organizations/org-123"));
        assert!(js.contains("Hello Claude"));
        assert!(js.contains("claude-sonnet-4-6"));
    }
}
