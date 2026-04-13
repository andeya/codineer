use crate::error::{AppError, AppResult};
use aineer_settings::schema::SettingsContent;
use aineer_settings::SettingsStore;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize)]
pub struct ModelGroup {
    pub provider: String,
    pub models: Vec<String>,
    pub available: bool,
}

/// Built-in provider definitions with well-known models.
/// Users can override or extend these via settings.json `providers` section.
fn builtin_providers() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        (
            "anthropic",
            vec![
                "claude-sonnet-4-20250514",
                "claude-opus-4-20250514",
                "claude-haiku-4-20250514",
            ],
        ),
        ("openai", vec!["gpt-4o", "gpt-4o-mini", "o3", "o4-mini"]),
        ("google", vec!["gemini-2.5-pro", "gemini-2.5-flash"]),
        ("deepseek", vec!["deepseek-chat", "deepseek-reasoner"]),
        ("xai", vec!["grok-3", "grok-3-mini"]),
        ("ollama", vec!["qwen3-coder", "llama3.1", "codellama"]),
        ("openrouter", vec![]),
        ("groq", vec![]),
    ]
}

/// Check whether a built-in provider has credentials configured.
fn is_builtin_available(name: &str, settings: &SettingsContent) -> bool {
    // Local providers are always available
    if name == "ollama" {
        return true;
    }

    let key_name = format!("{}_API_KEY", name.to_uppercase());

    // Check settings env section (keys saved via set_api_key)
    if let Some(env) = &settings.env {
        if env.get(&key_name).is_some_and(|v| !v.is_empty()) {
            return true;
        }
    }

    // Check providers section for inline api_key or api_key_env
    if let Some(providers) = &settings.providers {
        if let Some(cfg) = providers.get(name) {
            if cfg.api_key.as_ref().is_some_and(|k| !k.is_empty()) {
                return true;
            }
            if let Some(env_name) = &cfg.api_key_env {
                if std::env::var(env_name).is_ok() {
                    return true;
                }
                if let Some(env) = &settings.env {
                    if env.get(env_name).is_some_and(|v| !v.is_empty()) {
                        return true;
                    }
                }
            }
        }
    }

    // Check real environment variable
    std::env::var(&key_name)
        .ok()
        .is_some_and(|v| !v.is_empty())
}

pub fn build_model_groups(settings: &SettingsContent) -> Vec<ModelGroup> {
    let user_providers: BTreeMap<String, Vec<String>> = settings
        .providers
        .as_ref()
        .map(|p| {
            p.iter()
                .map(|(name, cfg)| (name.clone(), cfg.models.clone()))
                .collect()
        })
        .unwrap_or_default();

    let mut groups: Vec<ModelGroup> = Vec::new();
    let mut seen = HashSet::new();

    // Built-in providers first (user config can override their model lists)
    for (name, default_models) in builtin_providers() {
        seen.insert(name.to_string());
        let models = if let Some(user_models) = user_providers.get(name) {
            if user_models.is_empty() {
                default_models.iter().map(|s| s.to_string()).collect()
            } else {
                user_models.clone()
            }
        } else {
            default_models.iter().map(|s| s.to_string()).collect()
        };
        if !models.is_empty() {
            groups.push(ModelGroup {
                provider: name.to_string(),
                models,
                available: is_builtin_available(name, settings),
            });
        }
    }

    // User-defined custom providers (available if base_url is configured)
    for (name, models) in &user_providers {
        if !seen.contains(name) && !models.is_empty() {
            groups.push(ModelGroup {
                provider: name.clone(),
                models: models.clone(),
                available: true,
            });
        }
    }

    groups
}

pub struct ManagedSettings {
    store: Mutex<SettingsStore>,
}

impl ManagedSettings {
    pub fn load() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let user_path = home.join(".aineer").join("settings.json");
        let project_dir = std::env::current_dir().ok().map(|d| d.join(".aineer"));

        let store = SettingsStore::load(user_path, project_dir).unwrap_or_else(|e| {
            tracing::warn!("Failed to load settings: {e}, using defaults");
            SettingsStore::load(home.join(".aineer").join("settings.json"), None).unwrap_or_else(
                |_| {
                    SettingsStore::load(PathBuf::from("/dev/null"), None)
                        .expect("default settings must load")
                },
            )
        });

        Self {
            store: Mutex::new(store),
        }
    }

    pub fn merged(&self) -> Result<SettingsContent, String> {
        let store = self.store.lock().map_err(|e| e.to_string())?;
        Ok(store.merged().clone())
    }

    pub fn save_and_reload(&self, updates: &serde_json::Value) -> Result<(), String> {
        let store = self.store.lock().map_err(|e| e.to_string())?;
        store.save_user(updates).map_err(|e| e.to_string())?;
        drop(store);

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let user_path = home.join(".aineer").join("settings.json");
        let project_dir = std::env::current_dir().ok().map(|d| d.join(".aineer"));

        match SettingsStore::load(user_path, project_dir) {
            Ok(new_store) => {
                let mut guard = self.store.lock().map_err(|e| e.to_string())?;
                *guard = new_store;
            }
            Err(e) => {
                tracing::warn!("Failed to reload settings after save: {e}");
            }
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn get_settings(state: tauri::State<'_, ManagedSettings>) -> AppResult<SettingsContent> {
    state.merged().map_err(AppError::Settings)
}

#[tauri::command]
pub async fn update_settings(
    state: tauri::State<'_, ManagedSettings>,
    updates: serde_json::Value,
) -> AppResult<()> {
    state.save_and_reload(&updates).map_err(AppError::Settings)
}

#[tauri::command]
pub async fn get_api_key(
    state: tauri::State<'_, ManagedSettings>,
    provider: String,
) -> AppResult<Option<String>> {
    let store = state
        .store
        .lock()
        .map_err(|e| AppError::Settings(e.to_string()))?;
    let merged = store.merged();

    // Check env section first
    if let Some(env) = &merged.env {
        let key_name = format!("{}_API_KEY", provider.to_uppercase());
        if let Some(val) = env.get(&key_name) {
            return Ok(Some(val.clone()));
        }
    }

    // Check providers section for inline api_key
    if let Some(providers) = &merged.providers {
        if let Some(cfg) = providers.get(&provider) {
            if let Some(key) = &cfg.api_key {
                return Ok(Some(key.clone()));
            }
            // Check apiKeyEnv reference
            if let Some(env_name) = &cfg.api_key_env {
                if let Ok(val) = std::env::var(env_name) {
                    return Ok(Some(val));
                }
                if let Some(env) = &merged.env {
                    if let Some(val) = env.get(env_name) {
                        return Ok(Some(val.clone()));
                    }
                }
            }
        }
    }

    Ok(None)
}

#[tauri::command]
pub async fn list_model_groups(
    state: tauri::State<'_, ManagedSettings>,
) -> AppResult<Vec<ModelGroup>> {
    let merged = state.merged().map_err(AppError::Settings)?;
    let mut groups = build_model_groups(&merged);

    if let Some(handle) = crate::app_handle() {
        let engine = aineer_webai::WebAiEngine::new(handle.clone());
        let auth_set: HashSet<String> =
            aineer_webai::webauth::list_authenticated().into_iter().collect();

        for provider in engine.list_providers() {
            let short = provider.id.strip_suffix("-web").unwrap_or(&provider.id);
            let models: Vec<String> = engine
                .list_models(&provider.id)
                .into_iter()
                .map(|m| m.id)
                .collect();
            if !models.is_empty() {
                groups.push(ModelGroup {
                    provider: format!("webai/{short}"),
                    models,
                    available: auth_set.contains(&provider.id),
                });
            }
        }
    }

    Ok(groups)
}

#[tauri::command]
pub async fn set_api_key(
    state: tauri::State<'_, ManagedSettings>,
    provider: String,
    key: String,
) -> AppResult<()> {
    let key_name = format!("{}_API_KEY", provider.to_uppercase());
    let updates = serde_json::json!({
        "env": { key_name: key }
    });

    let store = state
        .store
        .lock()
        .map_err(|e| AppError::Settings(e.to_string()))?;
    store
        .save_user(&updates)
        .map_err(|e| AppError::Settings(e.to_string()))?;

    Ok(())
}
