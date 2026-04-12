use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::{WebAiError, WebAiResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProfile {
    pub provider_id: String,
    pub credentials: serde_json::Value,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthStore {
    pub profiles: HashMap<String, AuthProfile>,
}

fn store_path() -> PathBuf {
    let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join(".aineer").join("webai-credentials.json")
}

pub fn load() -> AuthStore {
    let path = store_path();
    match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => AuthStore::default(),
    }
}

pub fn save(store: &AuthStore) -> WebAiResult<()> {
    let path = store_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| WebAiError::Other(anyhow::anyhow!("create dir: {e}")))?;
    }
    let json = serde_json::to_string_pretty(store)?;
    std::fs::write(&path, json)
        .map_err(|e| WebAiError::Other(anyhow::anyhow!("write auth store: {e}")))?;
    Ok(())
}

pub fn get_credentials<T: serde::de::DeserializeOwned>(provider_id: &str) -> Option<T> {
    let store = load();
    let profile = store.profiles.get(provider_id)?;
    serde_json::from_value(profile.credentials.clone()).ok()
}

pub fn save_credentials(provider_id: &str, credentials: &impl Serialize) -> WebAiResult<()> {
    let mut store = load();
    store.profiles.insert(
        provider_id.to_string(),
        AuthProfile {
            provider_id: provider_id.to_string(),
            credentials: serde_json::to_value(credentials)?,
            updated_at: chrono::Utc::now().to_rfc3339(),
        },
    );
    save(&store)
}

pub fn list_authorized_providers() -> Vec<String> {
    load().profiles.keys().cloned().collect()
}

pub fn remove_credentials(provider_id: &str) -> WebAiResult<()> {
    let mut store = load();
    store.profiles.remove(provider_id);
    save(&store)
}
