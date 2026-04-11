pub mod credentials;

use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;

pub type ProviderId = String;
pub type ModelId = String;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SelectedModel {
    pub provider: ProviderId,
    pub model: ModelId,
}

impl std::fmt::Display for SelectedModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.provider, self.model)
    }
}

impl FromStr for SelectedModel {
    type Err = ProviderError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((provider, model)) = s.split_once('/') {
            Ok(Self {
                provider: provider.to_string(),
                model: model.to_string(),
            })
        } else {
            // Auto-detect: try well-known model prefixes
            let provider = detect_provider_from_model(s);
            Ok(Self {
                provider,
                model: s.to_string(),
            })
        }
    }
}

fn detect_provider_from_model(model: &str) -> String {
    let m = model.to_lowercase();
    if m.contains("claude") {
        return "anthropic".into();
    }
    if m.contains("gpt") || m.contains("o1") || m.contains("o3") || m.contains("o4") {
        return "openai".into();
    }
    if m.contains("gemini") {
        return "google".into();
    }
    if m.contains("grok") {
        return "xai".into();
    }
    if m.contains("deepseek") {
        return "deepseek".into();
    }
    if m.contains("mistral") || m.contains("mixtral") || m.contains("codestral") {
        return "mistral".into();
    }
    if m.contains("llama") || m.contains("qwen") || m.contains("phi") {
        return "ollama".into();
    }
    "openai".into()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelInfo {
    pub id: ModelId,
    pub display_name: String,
    pub max_tokens: Option<u32>,
    pub supports_vision: bool,
    pub supports_thinking: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Provider '{0}' not found")]
    NotFound(ProviderId),
    #[error("Not authenticated for {provider}: {message}")]
    NotAuthenticated {
        provider: ProviderId,
        message: String,
    },
    #[error("Model '{model}' not available for provider '{provider}'")]
    ModelNotAvailable {
        provider: ProviderId,
        model: ModelId,
    },
    #[error("API error: {0}")]
    Api(String),
    #[error("Credential error: {0}")]
    Credential(String),
    #[error("Invalid model format: {0}")]
    InvalidFormat(String),
}

#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn available_models(&self) -> Vec<ModelInfo>;
    fn is_authenticated(&self) -> bool;
    async fn authenticate(&self, credential: credentials::Credential) -> Result<(), ProviderError>;
}

pub struct ProviderRegistry {
    providers: BTreeMap<ProviderId, Arc<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, provider: Arc<dyn Provider>) {
        let id = provider.id().to_string();
        self.providers.insert(id, provider);
    }

    pub fn get(&self, id: &str) -> Option<&Arc<dyn Provider>> {
        self.providers.get(id)
    }

    pub fn list(&self) -> Vec<&Arc<dyn Provider>> {
        self.providers.values().collect()
    }

    pub fn auto_detect_default(&self) -> Result<SelectedModel, ProviderError> {
        // Prefer authenticated providers in order
        let preferred = ["anthropic", "openai", "google", "xai", "deepseek", "ollama"];
        for &pid in &preferred {
            if let Some(provider) = self.providers.get(pid) {
                if provider.is_authenticated() {
                    if let Some(model) = provider.available_models().first() {
                        return Ok(SelectedModel {
                            provider: pid.to_string(),
                            model: model.id.clone(),
                        });
                    }
                }
            }
        }
        Err(ProviderError::NotAuthenticated {
            provider: "any".into(),
            message: "No authenticated provider found. Configure an API key in Settings.".into(),
        })
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_model_from_str_with_provider() {
        let m: SelectedModel = "anthropic/claude-sonnet-4-20250514".parse().unwrap();
        assert_eq!(m.provider, "anthropic");
        assert_eq!(m.model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn selected_model_from_str_auto_detect() {
        let m: SelectedModel = "gpt-4o".parse().unwrap();
        assert_eq!(m.provider, "openai");
        assert_eq!(m.model, "gpt-4o");

        let m: SelectedModel = "claude-sonnet-4-6".parse().unwrap();
        assert_eq!(m.provider, "anthropic");
    }

    #[test]
    fn selected_model_display() {
        let m = SelectedModel {
            provider: "anthropic".into(),
            model: "claude-sonnet-4-6".into(),
        };
        assert_eq!(m.to_string(), "anthropic/claude-sonnet-4-6");
    }
}
