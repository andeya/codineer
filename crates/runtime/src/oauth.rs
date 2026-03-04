use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::config::OAuthConfig;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthTokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkceCodePair {
    pub verifier: String,
    pub challenge: String,
    pub challenge_method: PkceChallengeMethod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkceChallengeMethod {
    S256,
}

impl PkceChallengeMethod {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::S256 => "S256",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthAuthorizationRequest {
    pub authorize_url: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub state: String,
    pub code_challenge: String,
    pub code_challenge_method: PkceChallengeMethod,
    pub extra_params: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthTokenExchangeRequest {
    pub grant_type: &'static str,
    pub code: String,
    pub redirect_uri: String,
    pub client_id: String,
    pub code_verifier: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthRefreshRequest {
    pub grant_type: &'static str,
    pub refresh_token: String,
    pub client_id: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthCallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredOAuthCredentials {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_at: Option<u64>,
    #[serde(default)]
    scopes: Vec<String>,
}

impl From<OAuthTokenSet> for StoredOAuthCredentials {
    fn from(value: OAuthTokenSet) -> Self {
        Self {
            access_token: value.access_token,
            refresh_token: value.refresh_token,
            expires_at: value.expires_at,
            scopes: value.scopes,
        }
    }
}

impl From<StoredOAuthCredentials> for OAuthTokenSet {
    fn from(value: StoredOAuthCredentials) -> Self {
        Self {
            access_token: value.access_token,
            refresh_token: value.refresh_token,
            expires_at: value.expires_at,
            scopes: value.scopes,
        }
    }
}

impl OAuthAuthorizationRequest {
    #[must_use]
    pub fn from_config(
        config: &OAuthConfig,
        redirect_uri: impl Into<String>,
        state: impl Into<String>,
        pkce: &PkceCodePair,
    ) -> Self {
        Self {
            authorize_url: config.authorize_url.clone(),
            client_id: config.client_id.clone(),
            redirect_uri: redirect_uri.into(),
            scopes: config.scopes.clone(),
            state: state.into(),
            code_challenge: pkce.challenge.clone(),
            code_challenge_method: pkce.challenge_method,
            extra_params: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_extra_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_params.insert(key.into(), value.into());
        self
    }

    #[must_use]
    pub fn build_url(&self) -> String {
        let mut params = vec![
            ("response_type", "code".to_string()),
            ("client_id", self.client_id.clone()),
            ("redirect_uri", self.redirect_uri.clone()),
            ("scope", self.scopes.join(" ")),
            ("state", self.state.clone()),
            ("code_challenge", self.code_challenge.clone()),
            (
                "code_challenge_method",
                self.code_challenge_method.as_str().to_string(),
            ),
        ];
        params.extend(
            self.extra_params
                .iter()
                .map(|(key, value)| (key.as_str(), value.clone())),
        );
        let query = params
            .into_iter()
            .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(&value)))
            .collect::<Vec<_>>()
            .join("&");
        format!(
            "{}{}{}",
            self.authorize_url,
            if self.authorize_url.contains('?') {
                '&'
            } else {
                '?'
            },
            query
        )
    }
}

impl OAuthTokenExchangeRequest {
    #[must_use]
    pub fn from_config(
        config: &OAuthConfig,
        code: impl Into<String>,
        state: impl Into<String>,
        verifier: impl Into<String>,
        redirect_uri: impl Into<String>,
    ) -> Self {
        Self {
            grant_type: "authorization_code",
            code: code.into(),
            redirect_uri: redirect_uri.into(),
            client_id: config.client_id.clone(),
            code_verifier: verifier.into(),
            state: state.into(),
        }
    }

    #[must_use]
    pub fn form_params(&self) -> BTreeMap<&str, String> {
        BTreeMap::from([
            ("grant_type", self.grant_type.to_string()),
            ("code", self.code.clone()),
            ("redirect_uri", self.redirect_uri.clone()),
            ("client_id", self.client_id.clone()),
            ("code_verifier", self.code_verifier.clone()),
            ("state", self.state.clone()),
        ])
    }
}

impl OAuthRefreshRequest {
    #[must_use]
    pub fn from_config(
        config: &OAuthConfig,
        refresh_token: impl Into<String>,
        scopes: Option<Vec<String>>,
    ) -> Self {
        Self {
            grant_type: "refresh_token",
            refresh_token: refresh_token.into(),
            client_id: config.client_id.clone(),
            scopes: scopes.unwrap_or_else(|| config.scopes.clone()),
        }
    }

    #[must_use]
    pub fn form_params(&self) -> BTreeMap<&str, String> {
        BTreeMap::from([
            ("grant_type", self.grant_type.to_string()),
            ("refresh_token", self.refresh_token.clone()),
            ("client_id", self.client_id.clone()),
            ("scope", self.scopes.join(" ")),
        ])
    }
}

pub fn generate_pkce_pair() -> io::Result<PkceCodePair> {
    let verifier = generate_random_token(32)?;
    Ok(PkceCodePair {
        challenge: code_challenge_s256(&verifier),
        verifier,
        challenge_method: PkceChallengeMethod::S256,
    })
}

pub fn generate_state() -> io::Result<String> {
    generate_random_token(32)
}

