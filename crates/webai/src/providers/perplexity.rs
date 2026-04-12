use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::WebAiResult;
use crate::page::WebAiPage;
use crate::provider::{ModelInfo, ProviderConfig, WebProviderClient};

use super::dom_helpers::build_dom_send_js;

pub struct PerplexityWebProvider {
    config: ProviderConfig,
}

impl Default for PerplexityWebProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl PerplexityWebProvider {
    pub fn new() -> Self {
        Self {
            config: ProviderConfig {
                id: "perplexity-web".into(),
                name: "Perplexity (Web)".into(),
                start_url: "https://www.perplexity.ai".into(),
                host_key: "perplexity.ai".into(),
                models: vec![
                    ModelInfo {
                        id: "perplexity-web".into(),
                        name: "Perplexity (Sonar)".into(),
                        default: true,
                    },
                    ModelInfo {
                        id: "perplexity-pro".into(),
                        name: "Perplexity Pro".into(),
                        default: false,
                    },
                ],
            },
        }
    }
}

#[async_trait]
impl WebProviderClient for PerplexityWebProvider {
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
            "div[contenteditable=\"true\"]",
            "[role=\"textbox\"]",
            "textarea",
        ];
        let response_js = r#"
const nodes = Array.from(document.querySelectorAll('[class*="prose"], [class*="markdown"]'));
if (!nodes.length) return '';
return (nodes[nodes.length - 1].innerText || '').trim();
"#;
        let js = build_dom_send_js(message, &input_selectors, response_js, 3000, 120_000, 2);
        let text: String = page.evaluate(&js, None).await?;
        let (tx, rx) = mpsc::channel::<String>(256);
        tokio::spawn(async move {
            let _ = tx.send(text).await;
        });
        Ok(rx)
    }

    async fn check_session(&self, page: &WebAiPage) -> WebAiResult<bool> {
        page.evaluate::<bool>(
            "const r = await fetch('https://www.perplexity.ai/', { credentials: 'include' }); return r.ok;",
            None,
        )
        .await
    }
}
