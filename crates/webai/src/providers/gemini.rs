use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::WebAiResult;
use crate::page::WebAiPage;
use crate::provider::{ModelInfo, ProviderConfig, WebProviderClient};

use super::dom_helpers::build_dom_send_js;

pub struct GeminiWebProvider {
    config: ProviderConfig,
}

impl Default for GeminiWebProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiWebProvider {
    pub fn new() -> Self {
        Self {
            config: ProviderConfig {
                id: "gemini-web".into(),
                name: "Gemini Web".into(),
                start_url: "https://gemini.google.com/app".into(),
                host_key: "gemini.google.com".into(),
                models: vec![
                    ModelInfo {
                        id: "gemini-pro".into(),
                        name: "Gemini Pro (Web)".into(),
                        default: true,
                    },
                    ModelInfo {
                        id: "gemini-ultra".into(),
                        name: "Gemini Ultra (Web)".into(),
                        default: false,
                    },
                ],
            },
        }
    }
}

#[async_trait]
impl WebProviderClient for GeminiWebProvider {
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
        let input_selectors = [
            "textarea",
            "div[role=\"textbox\"]",
            "[contenteditable=\"true\"]",
        ];
        let response_js = r#"
const nodes = Array.from(document.querySelectorAll('.model-response, .markdown, .response-content'));
if (!nodes.length) return '';
const last = nodes[nodes.length - 1];
return (last.innerText || '').trim();
"#;
        let js = build_dom_send_js(message, &input_selectors, response_js, 2000, 120_000, 2);
        let text: String = page.evaluate(&js, None).await?;
        let (tx, rx) = mpsc::channel::<String>(256);
        tokio::spawn(async move {
            let _ = tx.send(text).await;
        });
        Ok(rx)
    }

    async fn check_session(&self, page: &WebAiPage) -> WebAiResult<bool> {
        page.evaluate::<bool>(
            "const r = await fetch('https://gemini.google.com/app', { credentials: 'include' }); return r.ok;",
            None,
        )
        .await
    }
}
