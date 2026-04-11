use crate::ProviderError;

#[derive(Debug, Clone)]
pub enum Credential {
    ApiKey(String),
    EnvVar(String),
    SystemKeychain(String),
    OAuth(OAuthToken),
}

#[derive(Debug, Clone)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
}

pub struct CredentialsManager {
    service_name: String,
}

impl CredentialsManager {
    pub fn new() -> Self {
        Self {
            service_name: "aineer".to_string(),
        }
    }

    /// Resolve credential from multiple sources (priority: EnvVar > Keychain > Manual)
    pub fn resolve(
        &self,
        provider_id: &str,
        env_var_name: Option<&str>,
    ) -> Result<String, ProviderError> {
        // 1. Try environment variable
        if let Some(var) = env_var_name {
            if let Ok(val) = std::env::var(var) {
                if !val.is_empty() {
                    return Ok(val);
                }
            }
        }

        // 2. Try well-known env vars
        let well_known = match provider_id {
            "anthropic" => Some("ANTHROPIC_API_KEY"),
            "openai" => Some("OPENAI_API_KEY"),
            "google" => Some("GOOGLE_API_KEY"),
            "xai" => Some("XAI_API_KEY"),
            "deepseek" => Some("DEEPSEEK_API_KEY"),
            "mistral" => Some("MISTRAL_API_KEY"),
            _ => None,
        };
        if let Some(var) = well_known {
            if let Ok(val) = std::env::var(var) {
                if !val.is_empty() {
                    return Ok(val);
                }
            }
        }

        // 3. Try system keychain
        match self.get_from_keychain(provider_id) {
            Ok(key) => return Ok(key),
            Err(_) => {}
        }

        Err(ProviderError::Credential(format!(
            "No credential found for provider '{}'. Set {} or configure in Settings.",
            provider_id,
            well_known.unwrap_or("the API key"),
        )))
    }

    pub fn store_in_keychain(&self, provider_id: &str, api_key: &str) -> Result<(), ProviderError> {
        let entry = keyring::Entry::new(&self.service_name, provider_id)
            .map_err(|e| ProviderError::Credential(e.to_string()))?;
        entry
            .set_password(api_key)
            .map_err(|e| ProviderError::Credential(e.to_string()))
    }

    pub fn get_from_keychain(&self, provider_id: &str) -> Result<String, ProviderError> {
        let entry = keyring::Entry::new(&self.service_name, provider_id)
            .map_err(|e| ProviderError::Credential(e.to_string()))?;
        entry
            .get_password()
            .map_err(|e| ProviderError::Credential(e.to_string()))
    }

    pub fn delete_from_keychain(&self, provider_id: &str) -> Result<(), ProviderError> {
        let entry = keyring::Entry::new(&self.service_name, provider_id)
            .map_err(|e| ProviderError::Credential(e.to_string()))?;
        entry
            .delete_credential()
            .map_err(|e| ProviderError::Credential(e.to_string()))
    }
}

impl Default for CredentialsManager {
    fn default() -> Self {
        Self::new()
    }
}
