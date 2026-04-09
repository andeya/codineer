use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use super::{CredentialError, CredentialResolver, ResolvedCredential};

/// Auto-discovers credentials from an existing Claude Code installation.
///
/// Searches the following locations (in order):
/// 1. macOS Keychain (`security find-generic-password -s "claude.ai" -w`)
/// 2. `~/.claude/.credentials.json` (`claudeAiOauth.accessToken`)
///
/// Checks `expiresAt` in the credentials file and rejects expired tokens
/// with a hint to run `claude login`.
///
/// This resolver does not support login/logout — Claude Code manages its own
/// credentials. Users must run `claude login` in Claude Code first.
#[derive(Debug, Clone)]
pub struct ClaudeCodeResolver {
    enabled: bool,
}

impl ClaudeCodeResolver {
    #[must_use]
    pub const fn new() -> Self {
        Self { enabled: true }
    }

    #[must_use]
    pub const fn with_enabled(enabled: bool) -> Self {
        Self { enabled }
    }
}

impl Default for ClaudeCodeResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct ClaudeCredentialsFile {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<ClaudeOAuthEntry>,
}

#[derive(Deserialize)]
struct ClaudeOAuthEntry {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<u64>,
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn claude_credentials_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(".claude"))
}

/// Token + optional expiry (millisecond epoch).
struct FileToken {
    access_token: String,
    expires_at: Option<u64>,
}

fn read_credentials_file() -> Option<FileToken> {
    let dir = claude_credentials_dir()?;
    let path = dir.join(".credentials.json");
    let contents = fs::read_to_string(path).ok()?;
    let parsed: ClaudeCredentialsFile = serde_json::from_str(&contents).ok()?;
    let entry = parsed.claude_ai_oauth?;
    let token = entry.access_token.filter(|t| !t.is_empty())?;
    Some(FileToken {
        access_token: token,
        expires_at: entry.expires_at,
    })
}

#[cfg(target_os = "macos")]
fn read_keychain() -> Option<String> {
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-s", "claude.ai", "-w"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if token.is_empty() {
        return None;
    }
    Some(token)
}

#[cfg(not(target_os = "macos"))]
fn read_keychain() -> Option<String> {
    None
}

impl CredentialResolver for ClaudeCodeResolver {
    fn id(&self) -> &str {
        "claude-code"
    }

    fn display_name(&self) -> &str {
        "Claude Code (auto-discover)"
    }

    fn priority(&self) -> u16 {
        300
    }

    fn resolve(&self) -> Result<Option<ResolvedCredential>, CredentialError> {
        if !self.enabled {
            return Ok(None);
        }

        // Keychain tokens don't carry expiry metadata — trust the OS store.
        if let Some(token) = read_keychain() {
            return Ok(Some(ResolvedCredential::BearerToken(token)));
        }

        if let Some(file_token) = read_credentials_file() {
            if let Some(expires_at) = file_token.expires_at {
                if expires_at <= now_epoch_ms() {
                    eprintln!(
                        "\x1b[33mwarning\x1b[0m: Claude Code token expired. \
                         Run `claude login` to refresh it."
                    );
                    return Ok(None);
                }
            }
            return Ok(Some(ResolvedCredential::BearerToken(
                file_token.access_token,
            )));
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::test_env_lock()
    }

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "claude-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn write_creds(tmp: &std::path::Path, json: &str) {
        let claude_dir = tmp.join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join(".credentials.json"), json).unwrap();
    }

    #[test]
    fn disabled_resolver_returns_none() {
        let resolver = ClaudeCodeResolver::with_enabled(false);
        assert_eq!(resolver.resolve().unwrap(), None);
    }

    #[test]
    fn returns_none_when_no_claude_dir() {
        let _guard = env_lock();
        let saved_home = std::env::var_os("HOME");
        let saved_profile = std::env::var_os("USERPROFILE");

        std::env::set_var("HOME", "/nonexistent-test-dir-12345");
        std::env::remove_var("USERPROFILE");

        let resolver = ClaudeCodeResolver::new();
        assert_eq!(resolver.resolve().unwrap(), None);

        if let Some(home) = saved_home {
            std::env::set_var("HOME", home);
        }
        if let Some(profile) = saved_profile {
            std::env::set_var("USERPROFILE", profile);
        }
    }

    #[test]
    fn reads_credentials_file() {
        let _guard = env_lock();
        let tmp = temp_dir("test");
        write_creds(
            &tmp,
            r#"{"claudeAiOauth":{"accessToken":"test-token-123"}}"#,
        );

        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", &tmp);

        let resolver = ClaudeCodeResolver::new();
        let cred = resolver.resolve().unwrap();
        assert_eq!(
            cred,
            Some(ResolvedCredential::BearerToken("test-token-123".into()))
        );

        if let Some(home) = saved_home {
            std::env::set_var("HOME", home);
        }
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn ignores_empty_token() {
        let _guard = env_lock();
        let tmp = temp_dir("empty");
        write_creds(&tmp, r#"{"claudeAiOauth":{"accessToken":""}}"#);

        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", &tmp);

        let resolver = ClaudeCodeResolver::new();
        assert_eq!(resolver.resolve().unwrap(), None);

        if let Some(home) = saved_home {
            std::env::set_var("HOME", home);
        }
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn returns_token_when_not_expired() {
        let _guard = env_lock();
        let tmp = temp_dir("notexpired");
        let future_ms = now_epoch_ms() + 3_600_000;
        write_creds(
            &tmp,
            &format!(
                r#"{{"claudeAiOauth":{{"accessToken":"fresh-tok","expiresAt":{future_ms}}}}}"#
            ),
        );

        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", &tmp);

        let resolver = ClaudeCodeResolver::new();
        let cred = resolver.resolve().unwrap();
        assert_eq!(
            cred,
            Some(ResolvedCredential::BearerToken("fresh-tok".into()))
        );

        if let Some(home) = saved_home {
            std::env::set_var("HOME", home);
        }
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn returns_none_when_token_expired() {
        let _guard = env_lock();
        let tmp = temp_dir("expired");
        write_creds(
            &tmp,
            r#"{"claudeAiOauth":{"accessToken":"old-tok","expiresAt":1000}}"#,
        );

        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", &tmp);

        let resolver = ClaudeCodeResolver::new();
        assert_eq!(resolver.resolve().unwrap(), None);

        if let Some(home) = saved_home {
            std::env::set_var("HOME", home);
        }
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn returns_token_when_no_expiry_field() {
        let _guard = env_lock();
        let tmp = temp_dir("noexpiry");
        write_creds(&tmp, r#"{"claudeAiOauth":{"accessToken":"no-expiry-tok"}}"#);

        let saved_home = std::env::var_os("HOME");
        std::env::set_var("HOME", &tmp);

        let resolver = ClaudeCodeResolver::new();
        let cred = resolver.resolve().unwrap();
        assert_eq!(
            cred,
            Some(ResolvedCredential::BearerToken("no-expiry-tok".into()))
        );

        if let Some(home) = saved_home {
            std::env::set_var("HOME", home);
        }
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn does_not_support_login() {
        let resolver = ClaudeCodeResolver::new();
        assert!(!resolver.supports_login());
    }

    #[test]
    fn metadata() {
        let resolver = ClaudeCodeResolver::new();
        assert_eq!(resolver.id(), "claude-code");
        assert_eq!(resolver.priority(), 300);
    }
}
