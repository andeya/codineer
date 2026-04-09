use super::{CredentialError, CredentialResolver, ResolvedCredential};

/// Resolves credentials from environment variables.
///
/// Supports a primary API key variable and an optional secondary bearer token variable.
/// When both are set, produces `ApiKeyAndBearer`. When only one is set, produces
/// the corresponding single-credential variant.
#[derive(Debug, Clone)]
pub struct EnvVarResolver {
    id: &'static str,
    display_name: &'static str,
    api_key_env: &'static str,
    bearer_env: Option<&'static str>,
    priority: u16,
}

impl EnvVarResolver {
    /// Generic constructor.
    #[must_use]
    pub const fn new(
        id: &'static str,
        display_name: &'static str,
        api_key_env: &'static str,
        bearer_env: Option<&'static str>,
        priority: u16,
    ) -> Self {
        Self {
            id,
            display_name,
            api_key_env,
            bearer_env,
            priority,
        }
    }

    /// Resolver for Anthropic credentials (`ANTHROPIC_API_KEY` + `ANTHROPIC_AUTH_TOKEN`).
    #[must_use]
    pub const fn anthropic() -> Self {
        Self::new(
            "env",
            "Environment Variables",
            "ANTHROPIC_API_KEY",
            Some("ANTHROPIC_AUTH_TOKEN"),
            100,
        )
    }

    /// Resolver for xAI credentials (`XAI_API_KEY`).
    #[must_use]
    pub const fn xai() -> Self {
        Self::new("env", "Environment Variables", "XAI_API_KEY", None, 100)
    }

    /// Resolver for OpenAI credentials (`OPENAI_API_KEY`).
    #[must_use]
    pub const fn openai() -> Self {
        Self::new("env", "Environment Variables", "OPENAI_API_KEY", None, 100)
    }
}

fn read_env_non_empty(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(value) if !value.is_empty() => Some(value),
        _ => None,
    }
}

impl CredentialResolver for EnvVarResolver {
    fn id(&self) -> &str {
        self.id
    }

    fn display_name(&self) -> &str {
        self.display_name
    }

    fn priority(&self) -> u16 {
        self.priority
    }

    fn resolve(&self) -> Result<Option<ResolvedCredential>, CredentialError> {
        let api_key = read_env_non_empty(self.api_key_env);
        let bearer = self.bearer_env.and_then(read_env_non_empty);

        match (api_key, bearer) {
            (Some(api_key), Some(bearer_token)) => Ok(Some(ResolvedCredential::ApiKeyAndBearer {
                api_key,
                bearer_token,
            })),
            (Some(api_key), None) => Ok(Some(ResolvedCredential::ApiKey(api_key))),
            (None, Some(bearer_token)) => Ok(Some(ResolvedCredential::BearerToken(bearer_token))),
            (None, None) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::test_env_lock()
    }

    #[test]
    fn anthropic_resolves_api_key() {
        let _guard = env_lock();
        std::env::set_var("ANTHROPIC_API_KEY", "sk-test");
        std::env::remove_var("ANTHROPIC_AUTH_TOKEN");

        let resolver = EnvVarResolver::anthropic();
        let cred = resolver.resolve().unwrap();
        assert_eq!(cred, Some(ResolvedCredential::ApiKey("sk-test".into())));

        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn anthropic_resolves_both() {
        let _guard = env_lock();
        std::env::set_var("ANTHROPIC_API_KEY", "sk-key");
        std::env::set_var("ANTHROPIC_AUTH_TOKEN", "bearer-tok");

        let resolver = EnvVarResolver::anthropic();
        let cred = resolver.resolve().unwrap();
        assert_eq!(
            cred,
            Some(ResolvedCredential::ApiKeyAndBearer {
                api_key: "sk-key".into(),
                bearer_token: "bearer-tok".into(),
            })
        );

        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
    }

    #[test]
    fn anthropic_returns_none_when_unset() {
        let _guard = env_lock();
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("ANTHROPIC_AUTH_TOKEN");

        let resolver = EnvVarResolver::anthropic();
        assert_eq!(resolver.resolve().unwrap(), None);
    }

    #[test]
    fn anthropic_bearer_only() {
        let _guard = env_lock();
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::set_var("ANTHROPIC_AUTH_TOKEN", "tok");

        let resolver = EnvVarResolver::anthropic();
        let cred = resolver.resolve().unwrap();
        assert_eq!(cred, Some(ResolvedCredential::BearerToken("tok".into())));

        std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
    }

    #[test]
    fn empty_values_treated_as_absent() {
        let _guard = env_lock();
        std::env::set_var("ANTHROPIC_API_KEY", "");
        std::env::set_var("ANTHROPIC_AUTH_TOKEN", "");

        let resolver = EnvVarResolver::anthropic();
        assert_eq!(resolver.resolve().unwrap(), None);

        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
    }

    #[test]
    fn xai_resolver_reads_correct_env() {
        let _guard = env_lock();
        std::env::set_var("XAI_API_KEY", "xai-test");

        let resolver = EnvVarResolver::xai();
        let cred = resolver.resolve().unwrap();
        assert_eq!(cred, Some(ResolvedCredential::ApiKey("xai-test".into())));
        assert_eq!(resolver.priority(), 100);

        std::env::remove_var("XAI_API_KEY");
    }

    #[test]
    fn does_not_support_login() {
        let resolver = EnvVarResolver::anthropic();
        assert!(!resolver.supports_login());
    }
}
