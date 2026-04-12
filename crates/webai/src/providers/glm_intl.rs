use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::WebAiResult;
use crate::page::WebAiPage;
use crate::provider::{ModelInfo, ProviderConfig, WebProviderClient};

use super::dom_helpers::build_dom_send_js;

pub struct GlmIntlWebProvider {
    config: ProviderConfig,
}

impl Default for GlmIntlWebProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GlmIntlWebProvider {
    pub fn new() -> Self {
        Self {
            config: ProviderConfig {
                id: "glm-intl-web".into(),
                name: "GLM International (Web)".into(),
                start_url: "https://chat.z.ai/".into(),
                host_key: "chat.z.ai".into(),
                models: vec![
                    ModelInfo {
                        id: "glm-4-plus".into(),
                        name: "GLM-4 Plus".into(),
                        default: true,
                    },
                    ModelInfo {
                        id: "glm-4-think".into(),
                        name: "GLM-4 Think".into(),
                        default: false,
                    },
                ],
            },
        }
    }
}

#[async_trait]
impl WebProviderClient for GlmIntlWebProvider {
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
        let input_selectors = ["textarea", "[contenteditable=\"true\"]", "input[type=text]"];
        let response_js = r#"
const nodes = Array.from(document.querySelectorAll('.chat-assistant'));
if (!nodes.length) return '';
return (nodes[nodes.length - 1].innerText || '').trim();
"#;
        let js = build_dom_send_js(message, &input_selectors, response_js, 900, 120_000, 3);
        let text: String = page.evaluate(&js, None).await?;
        let (tx, rx) = mpsc::channel::<String>(256);
        tokio::spawn(async move {
            let _ = tx.send(text).await;
        });
        Ok(rx)
    }

    async fn check_session(&self, page: &WebAiPage) -> WebAiResult<bool> {
        page.evaluate::<bool>(
            "const r = await fetch('https://chat.z.ai/', { credentials: 'include' }); return r.ok;",
            None,
        )
        .await
    }
}
