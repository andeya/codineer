use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::OAuthConfig;
use crate::oauth::{clear_oauth_credentials, load_oauth_credentials, save_oauth_credentials};

use super::{CredentialError, CredentialResolver, ResolvedCredential};

/// Callback type for refreshing an expired OAuth token.
/// The CLI layer provides this since it requires HTTP (api crate).
pub type RefreshFn = Arc<
    dyn Fn(
            &OAuthConfig,
            crate::OAuthTokenSet,
        ) -> Result<crate::OAuthTokenSet, Box<dyn std::error::Error + Send + Sync>>
        + Send
        + Sync,
>;

/// Callback type for running an interactive login flow.
/// The CLI layer provides this since it requires browser + HTTP.
pub type LoginFn = Arc<dyn Fn() -> Result<(), Box<dyn std::error::Error>> + Send + Sync>;

/// Resolves credentials from Aineer's saved OAuth tokens.
///
/// Loads tokens from the OS keyring or `~/.aineer/credentials.json`.
/// If the token is expired and a refresh callback is available, attempts refresh.
pub struct AineerOAuthResolver {
    oauth_config: Option<OAuthConfig>,
    refresh_fn: Option<RefreshFn>,
    login_fn: Option<LoginFn>,
}

impl std::fmt::Debug for AineerOAuthResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AineerOAuthResolver")
            .field("has_oauth_config", &self.oauth_config.is_some())
            .field("has_refresh_fn", &self.refresh_fn.is_some())
            .field("has_login_fn", &self.login_fn.is_some())
            .finish()
    }
}

impl AineerOAuthResolver {
    #[must_use]
    pub fn new(oauth_config: Option<OAuthConfig>) -> Self {
        Self {
            oauth_config,
            refresh_fn: None,
            login_fn: None,
        }
    }

    /// Set the callback used to refresh expired tokens.
    #[must_use]
    pub fn with_refresh_fn(mut self, f: RefreshFn) -> Self {
        self.refresh_fn = Some(f);
        self
    }

    /// Set the callback used for interactive login.
    #[must_use]
    pub fn with_login_fn(mut self, f: LoginFn) -> Self {
        self.login_fn = Some(f);
        self
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn is_expired(token_set: &crate::OAuthTokenSet) -> bool {
    token_set
        .expires_at
        .is_some_and(|expires_at| expires_at <= now_unix())
}

impl CredentialResolver for AineerOAuthResolver {
    fn id(&self) -> &str {
        "aineer-oauth"
    }

    fn display_name(&self) -> &str {
        "Aineer OAuth"
    }

    fn priority(&self) -> u16 {
        200
    }

    fn resolve(&self) -> Result<Option<ResolvedCredential>, CredentialError> {
        let token_set = load_oauth_credentials().map_err(|e| CredentialError::ResolverFailed {
            resolver_id: self.id().to_string(),
            source: Box::new(e),
        })?;

        let Some(token_set) = token_set else {
            return Ok(None);
        };

        if !is_expired(&token_set) {
            return Ok(Some(ResolvedCredential::BearerToken(
                token_set.access_token,
            )));
        }

        // Token is expired — try to refresh if we have both a config and a refresh callback
        if token_set.refresh_token.is_some() {
            if let (Some(config), Some(refresh)) = (&self.oauth_config, &self.refresh_fn) {
                match refresh(config, token_set) {
                    Ok(refreshed) => {
                        let _ = save_oauth_credentials(&refreshed);
                        return Ok(Some(ResolvedCredential::BearerToken(
                            refreshed.access_token,
                        )));
                    }
                    Err(e) => {
                        return Err(CredentialError::ResolverFailed {
                            resolver_id: self.id().to_string(),
                            source: e,
                        });
                    }
                }
            }
        }

        // Expired and can't refresh — not available
        Ok(None)
    }

    fn supports_login(&self) -> bool {
        self.login_fn.is_some()
    }

    fn login(&self) -> Result<(), Box<dyn std::error::Error>> {
        match &self.login_fn {
            Some(f) => f(),
            None => Err("OAuth login requires the CLI login flow; run `aineer login`".into()),
        }
    }

    fn logout(&self) -> Result<(), Box<dyn std::error::Error>> {
        clear_oauth_credentials().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::save_oauth_credentials;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::test_env_lock()
    }

    fn temp_config_home() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "oauth-resolver-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }

    #[test]
    fn returns_none_when_no_saved_credentials() {
        let _guard = env_lock();
        let config_home = temp_config_home();
        std::env::set_var("AINEER_CONFIG_HOME", &config_home);

        let resolver = AineerOAuthResolver::new(None);
        assert_eq!(resolver.resolve().unwrap(), None);

        std::env::remove_var("AINEER_CONFIG_HOME");
        let _ = std::fs::remove_dir_all(config_home);
    }

    #[test]
    fn returns_token_when_not_expired() {
        let _guard = env_lock();
        let config_home = temp_config_home();
        std::env::set_var("AINEER_CONFIG_HOME", &config_home);
        std::fs::create_dir_all(&config_home).unwrap();

        let future = now_unix() + 3600;
        let token_set = crate::OAuthTokenSet {
            access_token: "valid-token".into(),
            refresh_token: None,
            expires_at: Some(future),
            scopes: vec![],
        };
        save_oauth_credentials(&token_set).unwrap();

        let resolver = AineerOAuthResolver::new(None);
        let cred = resolver.resolve().unwrap();
        assert_eq!(
            cred,
            Some(ResolvedCredential::BearerToken("valid-token".into()))
        );

        clear_oauth_credentials().unwrap();
        std::env::remove_var("AINEER_CONFIG_HOME");
        let _ = std::fs::remove_dir_all(config_home);
    }

    #[test]
    fn returns_none_when_expired_without_refresh() {
        let _guard = env_lock();
        let config_home = temp_config_home();
        std::env::set_var("AINEER_CONFIG_HOME", &config_home);
        std::fs::create_dir_all(&config_home).unwrap();

        let token_set = crate::OAuthTokenSet {
            access_token: "expired".into(),
            refresh_token: None,
            expires_at: Some(1), // long expired
            scopes: vec![],
        };
        save_oauth_credentials(&token_set).unwrap();

        let resolver = AineerOAuthResolver::new(None);
        assert_eq!(resolver.resolve().unwrap(), None);

        clear_oauth_credentials().unwrap();
        std::env::remove_var("AINEER_CONFIG_HOME");
        let _ = std::fs::remove_dir_all(config_home);
    }

    #[test]
    fn logout_clears_credentials() {
        let _guard = env_lock();
        let config_home = temp_config_home();
        std::env::set_var("AINEER_CONFIG_HOME", &config_home);
        std::fs::create_dir_all(&config_home).unwrap();

        let future = now_unix() + 3600;
        let token_set = crate::OAuthTokenSet {
            access_token: "tok".into(),
            refresh_token: None,
            expires_at: Some(future),
            scopes: vec![],
        };
        save_oauth_credentials(&token_set).unwrap();

        let resolver = AineerOAuthResolver::new(None);
        assert!(resolver.resolve().unwrap().is_some());
        resolver.logout().unwrap();
        assert_eq!(resolver.resolve().unwrap(), None);

        std::env::remove_var("AINEER_CONFIG_HOME");
        let _ = std::fs::remove_dir_all(config_home);
    }

    #[test]
    fn supports_login_reflects_login_fn() {
        let resolver = AineerOAuthResolver::new(None);
        assert!(!resolver.supports_login());
        let resolver_with_fn = AineerOAuthResolver::new(None).with_login_fn(Arc::new(|| Ok(())));
        assert!(resolver_with_fn.supports_login());
    }

    #[test]
    fn login_without_handler_returns_error() {
        let resolver = AineerOAuthResolver::new(None);
        assert!(resolver.login().is_err());
    }

    #[test]
    fn metadata() {
        let resolver = AineerOAuthResolver::new(None);
        assert_eq!(resolver.id(), "aineer-oauth");
        assert_eq!(resolver.display_name(), "Aineer OAuth");
        assert_eq!(resolver.priority(), 200);
    }
}
