mod claude_code;
mod env_resolver;
pub mod oauth_resolver;

pub use claude_code::ClaudeCodeResolver;
pub use env_resolver::EnvVarResolver;
pub use oauth_resolver::AineerOAuthResolver;

pub use protocol::{CredentialStatus, ResolvedCredential};

use std::fmt;

/// Error type for credential resolution.
#[derive(Debug)]
pub enum CredentialError {
    /// No resolver in the chain produced a credential.
    NoCredentials {
        provider: &'static str,
        tried: Vec<String>,
    },
    /// An individual resolver encountered a fatal error.
    ResolverFailed {
        resolver_id: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCredentials { provider, tried } => {
                write!(
                    f,
                    "no credentials found for {provider} (tried: {})",
                    tried.join(", ")
                )
            }
            Self::ResolverFailed {
                resolver_id,
                source,
            } => {
                write!(f, "credential resolver '{resolver_id}' failed: {source}")
            }
        }
    }
}

impl std::error::Error for CredentialError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ResolverFailed { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

/// Trait for pluggable credential sources.
///
/// Each implementation represents one way to obtain API credentials
/// (e.g. environment variables, saved OAuth tokens, external tool discovery).
pub trait CredentialResolver: fmt::Debug + Send + Sync {
    /// Unique identifier for this resolver (e.g. `"env"`, `"aineer-oauth"`, `"claude-code"`).
    fn id(&self) -> &str;

    /// Human-readable name shown in UI (e.g. `"Environment Variables"`).
    fn display_name(&self) -> &str;

    /// Lower priority values are tried first. Convention:
    /// - 100: environment variables
    /// - 200: Aineer OAuth
    /// - 300: external tool discovery (Claude Code, etc.)
    fn priority(&self) -> u16;

    /// Attempt to resolve credentials. Returns `Ok(None)` if this source
    /// has no credentials (and the chain should try the next resolver).
    fn resolve(&self) -> Result<Option<ResolvedCredential>, CredentialError>;

    /// Whether this resolver supports interactive `login()`.
    fn supports_login(&self) -> bool {
        false
    }

    /// Run an interactive login flow. Only called when `supports_login()` is true.
    fn login(&self) -> Result<(), Box<dyn std::error::Error>> {
        Err("login not supported by this credential source".into())
    }

    /// Clear saved credentials. Only called when `supports_login()` is true.
    fn logout(&self) -> Result<(), Box<dyn std::error::Error>> {
        Err("logout not supported by this credential source".into())
    }
}

/// An ordered chain of credential resolvers for a single provider.
///
/// Resolvers are tried in priority order (lowest first). The first resolver
/// that returns `Some(credential)` wins.
pub struct CredentialChain {
    provider_name: &'static str,
    resolvers: Vec<Box<dyn CredentialResolver>>,
}

impl fmt::Debug for CredentialChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CredentialChain")
            .field("provider", &self.provider_name)
            .field("resolvers", &self.resolvers.len())
            .finish()
    }
}

impl CredentialChain {
    /// Build a chain for the given provider. Resolvers are sorted by priority.
    pub fn new(
        provider_name: &'static str,
        mut resolvers: Vec<Box<dyn CredentialResolver>>,
    ) -> Self {
        resolvers.sort_by_key(|r| r.priority());
        Self {
            provider_name,
            resolvers,
        }
    }

    /// Build an empty chain (always returns `NoCredentials`).
    #[must_use]
    pub fn empty(provider_name: &'static str) -> Self {
        Self {
            provider_name,
            resolvers: Vec::new(),
        }
    }

    /// Resolve credentials by walking the chain in priority order.
    pub fn resolve(&self) -> Result<ResolvedCredential, CredentialError> {
        let mut tried = Vec::new();
        for resolver in &self.resolvers {
            tried.push(resolver.display_name().to_string());
            match resolver.resolve() {
                Ok(Some(credential)) => return Ok(credential),
                Ok(None) => continue,
                Err(CredentialError::ResolverFailed { .. }) => continue,
                Err(error) => return Err(error),
            }
        }
        Err(CredentialError::NoCredentials {
            provider: self.provider_name,
            tried,
        })
    }

    /// Return status of each resolver in the chain.
    #[must_use]
    pub fn status(&self) -> Vec<CredentialStatus> {
        self.resolvers
            .iter()
            .map(|r| CredentialStatus {
                id: r.id().to_string(),
                display_name: r.display_name().to_string(),
                available: matches!(r.resolve(), Ok(Some(_))),
                supports_login: r.supports_login(),
            })
            .collect()
    }

    /// Return resolvers that support interactive login.
    pub fn login_sources(&self) -> Vec<&dyn CredentialResolver> {
        self.resolvers
            .iter()
            .filter(|r| r.supports_login())
            .map(|r| r.as_ref())
            .collect()
    }

    /// Find a resolver by id.
    pub fn get_resolver(&self, id: &str) -> Option<&dyn CredentialResolver> {
        self.resolvers
            .iter()
            .find(|r| r.id() == id)
            .map(|r| r.as_ref())
    }

    /// Iterate over all resolver IDs in priority order.
    pub fn resolver_ids(&self) -> impl Iterator<Item = &str> {
        self.resolvers.iter().map(|r| r.id())
    }

    /// Provider name this chain serves.
    #[must_use]
    pub fn provider_name(&self) -> &str {
        self.provider_name
    }

    /// Number of resolvers in the chain.
    #[must_use]
    pub fn len(&self) -> usize {
        self.resolvers.len()
    }

    /// Whether the chain has no resolvers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.resolvers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct StubResolver {
        id: &'static str,
        priority: u16,
        credential: Option<ResolvedCredential>,
        login_supported: bool,
    }

    impl CredentialResolver for StubResolver {
        fn id(&self) -> &str {
            self.id
        }
        fn display_name(&self) -> &str {
            self.id
        }
        fn priority(&self) -> u16 {
            self.priority
        }
        fn resolve(&self) -> Result<Option<ResolvedCredential>, CredentialError> {
            Ok(self.credential.clone())
        }
        fn supports_login(&self) -> bool {
            self.login_supported
        }
    }

    #[test]
    fn chain_resolves_first_available() {
        let chain = CredentialChain::new(
            "test",
            vec![
                Box::new(StubResolver {
                    id: "a",
                    priority: 200,
                    credential: Some(ResolvedCredential::ApiKey("key-a".into())),
                    login_supported: false,
                }),
                Box::new(StubResolver {
                    id: "b",
                    priority: 100,
                    credential: None,
                    login_supported: false,
                }),
            ],
        );
        // "b" has lower priority (tried first) but returns None, so "a" wins
        let cred = chain.resolve().expect("should resolve");
        assert_eq!(cred, ResolvedCredential::ApiKey("key-a".into()));
    }

    #[test]
    fn chain_sorts_by_priority() {
        let chain = CredentialChain::new(
            "test",
            vec![
                Box::new(StubResolver {
                    id: "high",
                    priority: 300,
                    credential: Some(ResolvedCredential::BearerToken("tok-high".into())),
                    login_supported: false,
                }),
                Box::new(StubResolver {
                    id: "low",
                    priority: 100,
                    credential: Some(ResolvedCredential::BearerToken("tok-low".into())),
                    login_supported: false,
                }),
            ],
        );
        let cred = chain.resolve().expect("should resolve");
        assert_eq!(cred, ResolvedCredential::BearerToken("tok-low".into()));
    }

    #[test]
    fn empty_chain_returns_no_credentials() {
        let chain = CredentialChain::empty("test");
        let err = chain.resolve().unwrap_err();
        assert!(matches!(err, CredentialError::NoCredentials { .. }));
        assert!(chain.is_empty());
    }

    #[test]
    fn chain_skips_none_resolvers() {
        let chain = CredentialChain::new(
            "test",
            vec![
                Box::new(StubResolver {
                    id: "empty1",
                    priority: 100,
                    credential: None,
                    login_supported: false,
                }),
                Box::new(StubResolver {
                    id: "empty2",
                    priority: 200,
                    credential: None,
                    login_supported: false,
                }),
                Box::new(StubResolver {
                    id: "found",
                    priority: 300,
                    credential: Some(ResolvedCredential::ApiKey("k".into())),
                    login_supported: false,
                }),
            ],
        );
        let cred = chain.resolve().expect("should resolve");
        assert_eq!(cred, ResolvedCredential::ApiKey("k".into()));
    }

    #[test]
    fn status_reports_all_resolvers() {
        let chain = CredentialChain::new(
            "test",
            vec![
                Box::new(StubResolver {
                    id: "env",
                    priority: 100,
                    credential: Some(ResolvedCredential::ApiKey("k".into())),
                    login_supported: false,
                }),
                Box::new(StubResolver {
                    id: "oauth",
                    priority: 200,
                    credential: None,
                    login_supported: true,
                }),
            ],
        );
        let statuses = chain.status();
        assert_eq!(statuses.len(), 2);
        assert!(statuses[0].available);
        assert!(!statuses[0].supports_login);
        assert!(!statuses[1].available);
        assert!(statuses[1].supports_login);
    }

    #[test]
    fn login_sources_filters_correctly() {
        let chain = CredentialChain::new(
            "test",
            vec![
                Box::new(StubResolver {
                    id: "env",
                    priority: 100,
                    credential: None,
                    login_supported: false,
                }),
                Box::new(StubResolver {
                    id: "oauth",
                    priority: 200,
                    credential: None,
                    login_supported: true,
                }),
            ],
        );
        let sources = chain.login_sources();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id(), "oauth");
    }

    #[test]
    fn get_resolver_finds_by_id() {
        let chain = CredentialChain::new(
            "test",
            vec![Box::new(StubResolver {
                id: "env",
                priority: 100,
                credential: None,
                login_supported: false,
            })],
        );
        assert!(chain.get_resolver("env").is_some());
        assert!(chain.get_resolver("nonexistent").is_none());
    }

    #[test]
    fn resolved_credential_debug_redacts() {
        let key = ResolvedCredential::ApiKey("secret".into());
        let debug = format!("{key:?}");
        assert!(!debug.contains("secret"));
        assert!(debug.contains("***"));
    }

    #[test]
    fn credential_error_display() {
        let err = CredentialError::NoCredentials {
            provider: "Anthropic",
            tried: vec!["env".into(), "oauth".into()],
        };
        let msg = err.to_string();
        assert!(msg.contains("Anthropic"));
        assert!(msg.contains("env, oauth"));
    }
}
