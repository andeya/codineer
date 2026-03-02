use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const DEFAULT_REMOTE_BASE_URL: &str = "https://api.anthropic.com";
pub const DEFAULT_SESSION_TOKEN_PATH: &str = "/run/ccr/session_token";
pub const DEFAULT_SYSTEM_CA_BUNDLE: &str = "/etc/ssl/certs/ca-certificates.crt";

pub const UPSTREAM_PROXY_ENV_KEYS: [&str; 8] = [
    "HTTPS_PROXY",
    "https_proxy",
    "NO_PROXY",
    "no_proxy",
    "SSL_CERT_FILE",
    "NODE_EXTRA_CA_CERTS",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
];

pub const NO_PROXY_HOSTS: [&str; 16] = [
    "localhost",
    "127.0.0.1",
    "::1",
    "169.254.0.0/16",
    "10.0.0.0/8",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "anthropic.com",
    ".anthropic.com",
    "*.anthropic.com",
    "github.com",
    "api.github.com",
    "*.github.com",
    "*.githubusercontent.com",
    "registry.npmjs.org",
    "index.crates.io",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSessionContext {
    pub enabled: bool,
    pub session_id: Option<String>,
    pub base_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamProxyBootstrap {
    pub remote: RemoteSessionContext,
    pub upstream_proxy_enabled: bool,
    pub token_path: PathBuf,
    pub ca_bundle_path: PathBuf,
    pub system_ca_path: PathBuf,
    pub token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamProxyState {
    pub enabled: bool,
    pub proxy_url: Option<String>,
    pub ca_bundle_path: Option<PathBuf>,
    pub no_proxy: String,
}

impl RemoteSessionContext {
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_env_map(&env::vars().collect())
    }

    #[must_use]
    pub fn from_env_map(env_map: &BTreeMap<String, String>) -> Self {
        Self {
            enabled: env_truthy(env_map.get("CODINEER_REMOTE")),
            session_id: env_map
                .get("CODINEER_REMOTE_SESSION_ID")
                .filter(|value| !value.is_empty())
                .cloned(),
            base_url: env_map
                .get("ANTHROPIC_BASE_URL")
                .filter(|value| !value.is_empty())
                .cloned()
                .unwrap_or_else(|| DEFAULT_REMOTE_BASE_URL.to_string()),
        }
    }
}

impl UpstreamProxyBootstrap {
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_env_map(&env::vars().collect())
    }

    #[must_use]
    pub fn from_env_map(env_map: &BTreeMap<String, String>) -> Self {
        let remote = RemoteSessionContext::from_env_map(env_map);
        let token_path = env_map
            .get("CCR_SESSION_TOKEN_PATH")
            .filter(|value| !value.is_empty())
            .map_or_else(|| PathBuf::from(DEFAULT_SESSION_TOKEN_PATH), PathBuf::from);
