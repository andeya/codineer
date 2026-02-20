use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use crate::json::JsonValue;
use crate::sandbox::{FilesystemIsolationMode, SandboxConfig};

pub const CODINEER_SETTINGS_SCHEMA_NAME: &str = "SettingsSchema";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigSource {
    User,
    Project,
    Local,
}

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
    enabled_plugins: BTreeMap<String, bool>,
    external_directories: Vec<String>,
    install_root: Option<String>,
    registry_path: Option<String>,
    bundled_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeFeatureConfig {
    hooks: RuntimeHookConfig,
    plugins: RuntimePluginConfig,
    mcp: McpConfigCollection,
    oauth: Option<OAuthConfig>,
    model: Option<String>,
    permission_mode: Option<ResolvedPermissionMode>,
    sandbox: SandboxConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeHookConfig {
    pre_tool_use: Vec<String>,
    post_tool_use: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct McpConfigCollection {
    servers: BTreeMap<String, ScopedMcpServerConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedMcpServerConfig {
    pub scope: ConfigSource,
    pub config: McpServerConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpTransport {
    Stdio,
    Sse,
