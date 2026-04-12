use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::{WebAiError, WebAiResult};
use crate::page::WebAiPage;
use crate::provider::{ModelInfo, ProviderConfig, WebProviderClient};

pub struct KimiProvider {
    config: ProviderConfig,
}

impl Default for KimiProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KimiProvider {
    pub fn new() -> Self {
        Self {
            config: ProviderConfig {
                id: "kimi-web".into(),
                name: "Kimi (Web)".into(),
                start_url: "https://www.kimi.com/".into(),
                host_key: "kimi.com".into(),
                models: vec![
                    ModelInfo {
                        id: "moonshot-v1-32k".into(),
                        name: "Moonshot v1 32K".into(),
                        default: true,
                    },
                    ModelInfo {
                        id: "moonshot-v1-8k".into(),
                        name: "Moonshot v1 8K".into(),
                        default: false,
                    },
                    ModelInfo {
                        id: "moonshot-v1-128k".into(),
                        name: "Moonshot v1 128K".into(),
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
            .unwrap_or("moonshot-v1-32k")
    }
}

#[async_trait]
impl WebProviderClient for KimiProvider {
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
        let text: String = page
            .evaluate(&js, None)
            .await
            .map_err(|e| WebAiError::Provider(format!("Kimi completion failed: {e}")))?;

        let (tx, rx) = mpsc::channel::<String>(1);
        tokio::spawn(async move {
            let _ = tx.send(text).await;
        });
        Ok(rx)
    }

    async fn check_session(&self, page: &WebAiPage) -> WebAiResult<bool> {
        page.evaluate::<bool>(
            "const r = await fetch('https://www.kimi.com/', { credentials: 'include', method: 'GET' }); return r.ok;",
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

function encodeConnectFrame(jsonStr) {{
  const enc = new TextEncoder().encode(jsonStr);
  const len = enc.length;
  const buf = new ArrayBuffer(5 + len);
  const u8 = new Uint8Array(buf);
  u8[0] = 0;
  u8[1] = (len >>> 24) & 0xff;
  u8[2] = (len >>> 16) & 0xff;
  u8[3] = (len >>> 8) & 0xff;
  u8[4] = len & 0xff;
  u8.set(enc, 5);
  return buf;
}}

function parseConnectFrames(arrayBuffer) {{
  const out = [];
  const u8 = new Uint8Array(arrayBuffer);
  let off = 0;
  const dec = new TextDecoder();
  while (off + 5 <= u8.length) {{
    const len = (u8[off + 1] << 24) | (u8[off + 2] << 16) | (u8[off + 3] << 8) | u8[off + 4];
    off += 5;
    if (len < 0 || off + len > u8.length) break;
    const slice = u8.subarray(off, off + len);
    off += len;
    try {{
      out.push(JSON.parse(dec.decode(slice)));
    }} catch (_e) {{}}
  }}
  return out;
}}

function extractAppendText(frames) {{
  let acc = '';
  for (const j of frames) {{
    if (!j || typeof j !== 'object') continue;
    const op = j.op;
    const block = j.block;
    const content = block && block.text && block.text.content;
    if (op === 'append' && content != null && typeof content === 'string') {{
      acc += content;
    }}
  }}
  return acc;
}}

const reqBody = JSON.stringify({{
  messages: [{{ role: 'user', content: message }}],
  model: model
}});

const res = await fetch('https://www.kimi.com/apiv2/kimi.gateway.chat.v1.ChatService/Chat', {{
  method: 'POST',
  headers: {{
    'Content-Type': 'application/connect+json',
    'Connect-Protocol-Version': '1'
  }},
  credentials: 'include',
  body: encodeConnectFrame(reqBody)
}});

if (!res.ok) {{
  const t = await res.text();
  throw new Error('[Kimi] ' + res.status + ' ' + t.slice(0, 500));
}}

const buf = await res.arrayBuffer();
const frames = parseConnectFrames(buf);
const text = extractAppendText(frames);
return text;
"#
    )
}
