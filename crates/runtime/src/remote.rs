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
        let system_ca_path = env_map
            .get("CCR_SYSTEM_CA_BUNDLE")
            .filter(|value| !value.is_empty())
            .map_or_else(|| PathBuf::from(DEFAULT_SYSTEM_CA_BUNDLE), PathBuf::from);
        let ca_bundle_path = env_map
            .get("CCR_CA_BUNDLE_PATH")
            .filter(|value| !value.is_empty())
            .map_or_else(default_ca_bundle_path, PathBuf::from);
        let token = read_token(&token_path).ok().flatten();

        Self {
            remote,
            upstream_proxy_enabled: env_truthy(env_map.get("CCR_UPSTREAM_PROXY_ENABLED")),
            token_path,
            ca_bundle_path,
            system_ca_path,
            token,
        }
    }

    #[must_use]
    pub fn should_enable(&self) -> bool {
        self.remote.enabled
            && self.upstream_proxy_enabled
            && self.remote.session_id.is_some()
            && self.token.is_some()
    }

    #[must_use]
    pub fn ws_url(&self) -> String {
        upstream_proxy_ws_url(&self.remote.base_url)
    }

    #[must_use]
    pub fn state_for_port(&self, port: u16) -> UpstreamProxyState {
        if !self.should_enable() {
            return UpstreamProxyState::disabled();
        }
        UpstreamProxyState {
            enabled: true,
            proxy_url: Some(format!("http://127.0.0.1:{port}")),
            ca_bundle_path: Some(self.ca_bundle_path.clone()),
            no_proxy: no_proxy_list(),
        }
    }
}

impl UpstreamProxyState {
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            proxy_url: None,
            ca_bundle_path: None,
            no_proxy: no_proxy_list(),
        }
    }

    #[must_use]
    pub fn subprocess_env(&self) -> BTreeMap<String, String> {
        if !self.enabled {
            return BTreeMap::new();
        }
        let Some(proxy_url) = &self.proxy_url else {
            return BTreeMap::new();
        };
        let Some(ca_bundle_path) = &self.ca_bundle_path else {
            return BTreeMap::new();
        };
        let ca_bundle_path = ca_bundle_path.to_string_lossy().into_owned();
        BTreeMap::from([
            ("HTTPS_PROXY".to_string(), proxy_url.clone()),
            ("https_proxy".to_string(), proxy_url.clone()),
            ("NO_PROXY".to_string(), self.no_proxy.clone()),
            ("no_proxy".to_string(), self.no_proxy.clone()),
            ("SSL_CERT_FILE".to_string(), ca_bundle_path.clone()),
            ("NODE_EXTRA_CA_CERTS".to_string(), ca_bundle_path.clone()),
            ("REQUESTS_CA_BUNDLE".to_string(), ca_bundle_path.clone()),
            ("CURL_CA_BUNDLE".to_string(), ca_bundle_path),
        ])
    }
}

pub fn read_token(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let token = contents.trim();
            if token.is_empty() {
                Ok(None)
            } else {
                Ok(Some(token.to_string()))
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[must_use]
pub fn upstream_proxy_ws_url(base_url: &str) -> String {
