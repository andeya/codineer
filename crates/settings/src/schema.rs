use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsContent {
    // General
    pub theme: Option<String>,
    pub font_size: Option<f32>,
    pub language: Option<String>,
    pub session_restore: Option<bool>,

    // Terminal
    pub terminal: Option<TerminalSettings>,

    // AI / Models
    pub model: Option<String>,
    pub model_aliases: Option<BTreeMap<String, String>>,
    pub fallback_models: Option<Vec<String>>,
    pub thinking_mode: Option<bool>,

    // Providers
    pub providers: Option<BTreeMap<String, CustomProviderConfig>>,

    // Gateway
    pub gateway: Option<GatewaySettings>,

    // Env
    pub env: Option<BTreeMap<String, String>>,

    // OAuth
    pub oauth: Option<OAuthConfig>,

    // Credentials
    pub credentials: Option<CredentialConfig>,

    // Permissions
    pub permission_mode: Option<String>,
    pub permissions: Option<PermissionsConfig>,

    // MCP Servers
    pub mcp_servers: Option<BTreeMap<String, McpServerConfig>>,

    // AI Rules
    pub rules: Option<RulesConfig>,

    // Hooks
    pub hooks: Option<HooksConfig>,

    // Plugins
    pub plugins: Option<PluginsConfig>,
    pub enabled_plugins: Option<BTreeMap<String, bool>>,

    // Sandbox
    pub sandbox: Option<SandboxConfig>,

    // Window behaviour
    pub close_to_tray: Option<bool>,

    // Cache auto-cleanup
    pub auto_cleanup: Option<AutoCleanupConfig>,

    // Advanced
    pub auto_compact: Option<bool>,
    pub max_context_tokens: Option<u32>,

    // Forward-compatible: preserve unknown keys
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TerminalSettings {
    pub shell_path: Option<String>,
    pub shell_args: Option<Vec<String>>,
    pub env: Option<BTreeMap<String, String>>,
    pub font_family: Option<String>,
    pub font_size: Option<f32>,
    pub cursor_shape: Option<String>,
    pub scrollback_lines: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomProviderConfig {
    pub base_url: String,
    pub api_version: Option<String>,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    pub models: Vec<String>,
    pub default_model: Option<String>,
    pub headers: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GatewaySettings {
    pub enabled: Option<bool>,
    pub listen_addr: Option<String>,
    pub default_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfig {
    pub client_id: String,
    pub authorize_url: String,
    pub token_url: String,
    pub callback_port: Option<u16>,
    pub manual_redirect_url: Option<String>,
    pub scopes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CredentialConfig {
    pub default_source: Option<String>,
    pub auto_discover: Option<bool>,
    pub claude_code: Option<ClaudeCodeConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClaudeCodeConfig {
    pub enabled: Option<bool>,
    pub config_path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PermissionsConfig {
    pub rules: Option<Vec<PermissionRule>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRule {
    pub pattern: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum McpServerConfig {
    Stdio {
        command: String,
        args: Option<Vec<String>>,
        env: Option<BTreeMap<String, String>>,
        description: Option<String>,
    },
    Sse {
        url: String,
        headers: Option<BTreeMap<String, String>>,
        description: Option<String>,
    },
    Http {
        url: String,
        headers: Option<BTreeMap<String, String>>,
        description: Option<String>,
    },
    Ws {
        url: String,
        headers: Option<BTreeMap<String, String>>,
        description: Option<String>,
    },
    Sdk {
        name: String,
        description: Option<String>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RulesConfig {
    pub auto_inject_budget: Option<u32>,
    pub rules_dir: Option<String>,
    pub specs_dir: Option<String>,
    pub disable_auto_inject: Option<bool>,
    pub disabled_rules: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HooksConfig {
    pub pre_tool_use: Option<Vec<String>>,
    pub post_tool_use: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginsConfig {
    pub external_dir: Option<String>,
}

/// Schedule-based cache auto-cleanup.
/// `interval`: "off" | "daily" | "weekly" | "monthly"
/// `last_run_ms`: epoch millis of the last successful run (managed by the app).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AutoCleanupConfig {
    pub interval: Option<String>,
    pub target: Option<String>,
    pub last_run_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SandboxConfig {
    pub enabled: Option<bool>,
    pub namespace_restrictions: Option<bool>,
    pub network_isolation: Option<bool>,
    pub filesystem_mode: Option<String>,
    pub allowed_mounts: Option<Vec<String>>,
}
