use std::collections::BTreeMap;
use std::path::PathBuf;

pub use protocol::GeminiCacheConfig;

use crate::json::JsonValue;
use crate::permissions::PermissionRule;
use crate::sandbox::SandboxConfig;

pub use mcp::{
    McpConfigCollection, McpManagedProxyServerConfig, McpOAuthConfig, McpRemoteServerConfig,
    McpSdkServerConfig, McpServerConfig, McpStdioServerConfig, McpTransport,
    McpWebSocketServerConfig, ScopedMcpServerConfig,
};
pub use protocol::config::ConfigSource;

pub const AINEER_SETTINGS_SCHEMA_NAME: &str = "SettingsSchema";

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedPermissionMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigEntry {
    pub source: ConfigSource,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    merged: BTreeMap<String, JsonValue>,
    loaded_entries: Vec<ConfigEntry>,
    feature_config: RuntimeFeatureConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimePluginConfig {
    pub(crate) enabled_plugins: BTreeMap<String, bool>,
    pub(crate) external_directories: Vec<String>,
    pub(crate) install_root: Option<String>,
    pub(crate) registry_path: Option<String>,
    pub(crate) bundled_root: Option<String>,
}

/// Controls which external credential sources are enabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialConfig {
    /// Default auth source id for `aineer login` (e.g. `"aineer-oauth"`).
    pub default_source: Option<String>,
    /// Whether to auto-discover credentials from external tools (default: true).
    pub auto_discover: bool,
    /// Enable Claude Code credential auto-discovery.
    pub claude_code_enabled: bool,
}

impl Default for CredentialConfig {
    fn default() -> Self {
        Self {
            default_source: None,
            auto_discover: true,
            claude_code_enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeFeatureConfig {
    pub(crate) hooks: RuntimeHookConfig,
    pub(crate) plugins: RuntimePluginConfig,
    pub(crate) mcp: McpConfigCollection,
    pub(crate) oauth: Option<OAuthConfig>,
    pub(crate) model: Option<String>,
    pub(crate) fallback_models: Vec<String>,
    pub(crate) model_aliases: BTreeMap<String, String>,
    pub(crate) permission_mode: Option<ResolvedPermissionMode>,
    pub(crate) permission_rules: Vec<PermissionRule>,
    pub(crate) sandbox: SandboxConfig,
    pub(crate) providers: BTreeMap<String, CustomProviderConfig>,
    pub(crate) credentials: CredentialConfig,
    pub(crate) gemini_cache: GeminiCacheConfig,
}

/// Configuration for a custom OpenAI-compatible provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomProviderConfig {
    pub base_url: String,
    /// Appended as query on `.../chat/completions` when the server requires a version parameter (e.g. `api-version=...`).
    pub api_version: Option<String>,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    pub models: Vec<String>,
    pub default_model: Option<String>,
    /// Extra HTTP headers sent with every request to this provider.
    pub headers: BTreeMap<String, String>,
}

pub use protocol::hook_config::RuntimeHookConfig;
pub use protocol::OAuthConfig;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Parse(String),
}

impl RuntimeConfig {
    #[must_use]
    pub fn new(
        merged: BTreeMap<String, JsonValue>,
        loaded_entries: Vec<ConfigEntry>,
        feature_config: RuntimeFeatureConfig,
    ) -> Self {
        Self {
            merged,
            loaded_entries,
            feature_config,
        }
    }

    #[must_use]
    pub fn empty() -> Self {
        Self::new(BTreeMap::new(), Vec::new(), RuntimeFeatureConfig::default())
    }

    #[must_use]
    pub fn merged(&self) -> &BTreeMap<String, JsonValue> {
        &self.merged
    }

    #[must_use]
    pub fn loaded_entries(&self) -> &[ConfigEntry] {
        &self.loaded_entries
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        self.merged.get(key)
    }

    #[must_use]
    pub fn as_json(&self) -> JsonValue {
        JsonValue::Object(self.merged.clone())
    }

    #[must_use]
    pub fn feature_config(&self) -> &RuntimeFeatureConfig {
        &self.feature_config
    }

    #[must_use]
    pub fn mcp(&self) -> &McpConfigCollection {
        &self.feature_config.mcp
    }

    #[must_use]
    pub fn hooks(&self) -> &RuntimeHookConfig {
        &self.feature_config.hooks
    }

    #[must_use]
    pub fn plugins(&self) -> &RuntimePluginConfig {
        &self.feature_config.plugins
    }

    #[must_use]
    pub fn oauth(&self) -> Option<&OAuthConfig> {
        self.feature_config.oauth.as_ref()
    }

    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.feature_config.model.as_deref()
    }

    #[must_use]
    pub fn fallback_models(&self) -> &[String] {
        &self.feature_config.fallback_models
    }

    #[must_use]
    pub fn model_aliases(&self) -> &BTreeMap<String, String> {
        &self.feature_config.model_aliases
    }

    #[must_use]
    pub fn permission_mode(&self) -> Option<ResolvedPermissionMode> {
        self.feature_config.permission_mode
    }

    #[must_use]
    pub fn permission_rules(&self) -> &[PermissionRule] {
        &self.feature_config.permission_rules
    }

    #[must_use]
    pub fn sandbox(&self) -> &SandboxConfig {
        &self.feature_config.sandbox
    }

    #[must_use]
    pub fn providers(&self) -> &BTreeMap<String, CustomProviderConfig> {
        &self.feature_config.providers
    }

    #[must_use]
    pub fn credentials(&self) -> &CredentialConfig {
        &self.feature_config.credentials
    }

    #[must_use]
    pub fn gemini_cache(&self) -> &GeminiCacheConfig {
        &self.feature_config.gemini_cache
    }

    /// Return the `"env"` section from merged config as key-value pairs.
    /// Callers can use this to apply environment variables to the process.
    #[must_use]
    pub fn env_section(&self) -> Vec<(String, String)> {
        self.merged
            .get("env")
            .and_then(JsonValue::as_object)
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Resolve an environment variable by name.
    ///
    /// Lookup order:
    /// 1. System environment variable (`std::env::var`)
    /// 2. The `"env"` section in merged settings.json
    ///
    /// Returns `None` if the key is absent or empty in both sources.
    #[must_use]
    pub fn resolve_env(&self, key: &str) -> Option<String> {
        if let Ok(val) = std::env::var(key) {
            if !val.is_empty() {
                return Some(val);
            }
        }
        self.merged
            .get("env")
            .and_then(JsonValue::as_object)
            .and_then(|obj| obj.get(key))
            .and_then(JsonValue::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    /// Batch-apply the `"env"` section to the process environment.
    ///
    /// Only sets variables not already present so explicit shell exports take
    /// precedence. Call once at startup to make `std::env::var` work for
    /// downstream code that is not config-aware.
    pub fn apply_env(&self) {
        for (key, value) in self.env_section() {
            if std::env::var_os(&key).is_none() {
                std::env::set_var(&key, &value);
            }
        }
    }
}

impl RuntimeFeatureConfig {
    #[must_use]
    pub fn with_hooks(mut self, hooks: RuntimeHookConfig) -> Self {
        self.hooks = hooks;
        self
    }

    #[must_use]
    pub fn with_plugins(mut self, plugins: RuntimePluginConfig) -> Self {
        self.plugins = plugins;
        self
    }

    #[must_use]
    pub fn hooks(&self) -> &RuntimeHookConfig {
        &self.hooks
    }

    #[must_use]
    pub fn plugins(&self) -> &RuntimePluginConfig {
        &self.plugins
    }

    #[must_use]
    pub fn mcp(&self) -> &McpConfigCollection {
        &self.mcp
    }

    #[must_use]
    pub fn oauth(&self) -> Option<&OAuthConfig> {
        self.oauth.as_ref()
    }

    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    #[must_use]
    pub fn permission_mode(&self) -> Option<ResolvedPermissionMode> {
        self.permission_mode
    }

    #[must_use]
    pub fn permission_rules(&self) -> &[PermissionRule] {
        &self.permission_rules
    }

    #[must_use]
    pub fn sandbox(&self) -> &SandboxConfig {
        &self.sandbox
    }

    #[must_use]
    pub fn providers(&self) -> &BTreeMap<String, CustomProviderConfig> {
        &self.providers
    }

    #[must_use]
    pub fn credentials(&self) -> &CredentialConfig {
        &self.credentials
    }

    #[must_use]
    pub fn gemini_cache(&self) -> &GeminiCacheConfig {
        &self.gemini_cache
    }

    /// Set the custom providers map (useful in tests and programmatic construction).
    pub fn set_providers(&mut self, providers: BTreeMap<String, CustomProviderConfig>) {
        self.providers = providers;
    }

    pub fn set_fallback_models(&mut self, fallback_models: Vec<String>) {
        self.fallback_models = fallback_models;
    }

    pub fn set_model_aliases(&mut self, aliases: BTreeMap<String, String>) {
        self.model_aliases = aliases;
    }
}

impl RuntimePluginConfig {
    #[must_use]
    pub fn enabled_plugins(&self) -> &BTreeMap<String, bool> {
        &self.enabled_plugins
    }

    #[must_use]
    pub fn external_directories(&self) -> &[String] {
        &self.external_directories
    }

    #[must_use]
    pub fn install_root(&self) -> Option<&str> {
        self.install_root.as_deref()
    }

    #[must_use]
    pub fn registry_path(&self) -> Option<&str> {
        self.registry_path.as_deref()
    }

    #[must_use]
    pub fn bundled_root(&self) -> Option<&str> {
        self.bundled_root.as_deref()
    }

    pub fn set_plugin_state(&mut self, plugin_id: String, enabled: bool) {
        self.enabled_plugins.insert(plugin_id, enabled);
    }

    #[must_use]
    pub fn state_for(&self, plugin_id: &str, default_enabled: bool) -> bool {
        self.enabled_plugins
            .get(plugin_id)
            .copied()
            .unwrap_or(default_enabled)
    }
}
