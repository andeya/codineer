use super::{ALT_REQUEST_ID_HEADER, REQUEST_ID_HEADER};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use runtime::{clear_oauth_credentials, save_oauth_credentials, OAuthConfig};

use super::{
    now_unix_timestamp, oauth_token_is_expired, resolve_saved_oauth_token,
    resolve_startup_auth_source, AuthSource, CodineerApiClient, OAuthTokenSet,
};
use crate::providers::RetryPolicy;
use crate::types::{ContentBlockDelta, MessageRequest};

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn temp_config_home() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "api-oauth-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ))
}

fn cleanup_temp_config_home(config_home: &std::path::Path) {
    match std::fs::remove_dir_all(config_home) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("cleanup temp dir: {error}"),
    }
}

fn sample_oauth_config(token_url: String) -> OAuthConfig {
    OAuthConfig {
        client_id: "runtime-client".to_string(),
        authorize_url: "https://console.test/oauth/authorize".to_string(),
        token_url,
        callback_port: Some(4545),
        manual_redirect_url: Some("https://console.test/oauth/callback".to_string()),
        scopes: vec!["org:read".to_string(), "user:write".to_string()],
    }
}

fn spawn_token_server(response_body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("local addr");
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept connection");
        let mut buffer = [0_u8; 4096];
        let _ = stream.read(&mut buffer).expect("read request");
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    });
    format!("http://{address}/oauth/token")
}

#[test]
fn read_api_key_requires_presence() {
    let _guard = env_lock();
    let config_home = temp_config_home();
    std::env::set_var("CODINEER_CONFIG_HOME", &config_home);
    std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
    std::env::remove_var("ANTHROPIC_API_KEY");
    let error = super::read_api_key().expect_err("missing key should error");
    assert!(matches!(
        error,
        crate::error::ApiError::MissingCredentials { .. }
    ));
    std::env::remove_var("CODINEER_CONFIG_HOME");
    cleanup_temp_config_home(&config_home);
}

#[test]
fn read_api_key_requires_non_empty_value() {
    let _guard = env_lock();
    let config_home = temp_config_home();
    std::env::set_var("CODINEER_CONFIG_HOME", &config_home);
    std::env::set_var("ANTHROPIC_AUTH_TOKEN", "");
    std::env::remove_var("ANTHROPIC_API_KEY");
    let error = super::read_api_key().expect_err("empty key should error");
    assert!(matches!(
        error,
        crate::error::ApiError::MissingCredentials { .. }
    ));
    std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
    std::env::remove_var("CODINEER_CONFIG_HOME");
    cleanup_temp_config_home(&config_home);
}

#[test]
fn read_api_key_prefers_api_key_env() {
    let _guard = env_lock();
    std::env::set_var("ANTHROPIC_AUTH_TOKEN", "auth-token");
    std::env::set_var("ANTHROPIC_API_KEY", "legacy-key");
    assert_eq!(
        super::read_api_key().expect("api key should load"),
        "legacy-key"
    );
    std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
    std::env::remove_var("ANTHROPIC_API_KEY");
}

#[test]
fn read_auth_token_reads_auth_token_env() {
    let _guard = env_lock();
    std::env::set_var("ANTHROPIC_AUTH_TOKEN", "auth-token");
    assert_eq!(super::read_auth_token().as_deref(), Some("auth-token"));
    std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
}

#[test]
fn oauth_token_maps_to_bearer_auth_source() {
    let auth = AuthSource::from(OAuthTokenSet {
        access_token: "access-token".to_string(),
        refresh_token: Some("refresh".to_string()),
        expires_at: Some(123),
        scopes: vec!["scope:a".to_string()],
    });
    assert_eq!(auth.bearer_token(), Some("access-token"));
    assert_eq!(auth.api_key(), None);
}

#[test]
fn auth_source_from_env_combines_api_key_and_bearer_token() {
    let _guard = env_lock();
    std::env::set_var("ANTHROPIC_AUTH_TOKEN", "auth-token");
    std::env::set_var("ANTHROPIC_API_KEY", "legacy-key");
    let auth = AuthSource::from_env().expect("env auth");
    assert_eq!(auth.api_key(), Some("legacy-key"));
    assert_eq!(auth.bearer_token(), Some("auth-token"));
    std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
    std::env::remove_var("ANTHROPIC_API_KEY");
}

#[test]
fn auth_source_from_saved_oauth_when_env_absent() {
    let _guard = env_lock();
    let config_home = temp_config_home();
    std::env::set_var("CODINEER_CONFIG_HOME", &config_home);
    std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
    std::env::remove_var("ANTHROPIC_API_KEY");
    save_oauth_credentials(&runtime::OAuthTokenSet {
        access_token: "saved-access-token".to_string(),
        refresh_token: Some("refresh".to_string()),
        expires_at: Some(now_unix_timestamp() + 300),
        scopes: vec!["scope:a".to_string()],
    })
    .expect("save oauth credentials");

    let auth = AuthSource::from_env_or_saved().expect("saved auth");
    assert_eq!(auth.bearer_token(), Some("saved-access-token"));

    clear_oauth_credentials().expect("clear credentials");
    std::env::remove_var("CODINEER_CONFIG_HOME");
    cleanup_temp_config_home(&config_home);
}

#[test]
fn oauth_token_expiry_uses_expires_at_timestamp() {
    assert!(oauth_token_is_expired(&OAuthTokenSet {
        access_token: "access-token".to_string(),
        refresh_token: None,
        expires_at: Some(1),
        scopes: Vec::new(),
    }));
    assert!(!oauth_token_is_expired(&OAuthTokenSet {
        access_token: "access-token".to_string(),
        refresh_token: None,
        expires_at: Some(now_unix_timestamp() + 60),
        scopes: Vec::new(),
    }));
}

#[test]
fn resolve_saved_oauth_token_refreshes_expired_credentials() {
    let _guard = env_lock();
    let config_home = temp_config_home();
    std::env::set_var("CODINEER_CONFIG_HOME", &config_home);
    std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
    std::env::remove_var("ANTHROPIC_API_KEY");
    save_oauth_credentials(&runtime::OAuthTokenSet {
        access_token: "expired-access-token".to_string(),
        refresh_token: Some("refresh-token".to_string()),
        expires_at: Some(1),
        scopes: vec!["scope:a".to_string()],
    })
    .expect("save expired oauth credentials");

    let token_url = spawn_token_server(
        "{\"access_token\":\"refreshed-token\",\"refresh_token\":\"fresh-refresh\",\"expires_at\":9999999999,\"scopes\":[\"scope:a\"]}",
    );
    let resolved = resolve_saved_oauth_token(&sample_oauth_config(token_url))
        .expect("resolve refreshed token")
        .expect("token set present");
    assert_eq!(resolved.access_token, "refreshed-token");
    let stored = runtime::load_oauth_credentials()
        .expect("load stored credentials")
        .expect("stored token set");
    assert_eq!(stored.access_token, "refreshed-token");

    clear_oauth_credentials().expect("clear credentials");
    std::env::remove_var("CODINEER_CONFIG_HOME");
    cleanup_temp_config_home(&config_home);
}

#[test]
fn resolve_startup_auth_source_uses_saved_oauth_without_loading_config() {
    let _guard = env_lock();
    let config_home = temp_config_home();
    std::env::set_var("CODINEER_CONFIG_HOME", &config_home);
    std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
    std::env::remove_var("ANTHROPIC_API_KEY");
    save_oauth_credentials(&runtime::OAuthTokenSet {
        access_token: "saved-access-token".to_string(),
        refresh_token: Some("refresh".to_string()),
        expires_at: Some(now_unix_timestamp() + 300),
        scopes: vec!["scope:a".to_string()],
    })
    .expect("save oauth credentials");

    let auth = resolve_startup_auth_source(|| panic!("config should not be loaded"))
        .expect("startup auth");
    assert_eq!(auth.bearer_token(), Some("saved-access-token"));

    clear_oauth_credentials().expect("clear credentials");
    std::env::remove_var("CODINEER_CONFIG_HOME");
    cleanup_temp_config_home(&config_home);
}

#[test]
fn resolve_startup_auth_source_errors_when_refreshable_token_lacks_config() {
    let _guard = env_lock();
    let config_home = temp_config_home();
    std::env::set_var("CODINEER_CONFIG_HOME", &config_home);
    std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
    std::env::remove_var("ANTHROPIC_API_KEY");
    save_oauth_credentials(&runtime::OAuthTokenSet {
        access_token: "expired-access-token".to_string(),
        refresh_token: Some("refresh-token".to_string()),
        expires_at: Some(1),
        scopes: vec!["scope:a".to_string()],
    })
    .expect("save expired oauth credentials");

    let error = resolve_startup_auth_source(|| Ok(None)).expect_err("missing config should error");
    assert!(
        matches!(error, crate::error::ApiError::Auth(message) if message.contains("runtime OAuth config is missing"))
    );

    let stored = runtime::load_oauth_credentials()
        .expect("load stored credentials")
        .expect("stored token set");
    assert_eq!(stored.access_token, "expired-access-token");
    assert_eq!(stored.refresh_token.as_deref(), Some("refresh-token"));

    clear_oauth_credentials().expect("clear credentials");
    std::env::remove_var("CODINEER_CONFIG_HOME");
    cleanup_temp_config_home(&config_home);
}

#[test]
fn resolve_saved_oauth_token_preserves_refresh_token_when_refresh_response_omits_it() {
    let _guard = env_lock();
    let config_home = temp_config_home();
    std::env::set_var("CODINEER_CONFIG_HOME", &config_home);
    std::env::remove_var("ANTHROPIC_AUTH_TOKEN");
    std::env::remove_var("ANTHROPIC_API_KEY");
    save_oauth_credentials(&runtime::OAuthTokenSet {
        access_token: "expired-access-token".to_string(),
        refresh_token: Some("refresh-token".to_string()),
        expires_at: Some(1),
        scopes: vec!["scope:a".to_string()],
    })
    .expect("save expired oauth credentials");

    let token_url = spawn_token_server(
        "{\"access_token\":\"refreshed-token\",\"expires_at\":9999999999,\"scopes\":[\"scope:a\"]}",
    );
    let resolved = resolve_saved_oauth_token(&sample_oauth_config(token_url))
        .expect("resolve refreshed token")
        .expect("token set present");
    assert_eq!(resolved.access_token, "refreshed-token");
    assert_eq!(resolved.refresh_token.as_deref(), Some("refresh-token"));
    let stored = runtime::load_oauth_credentials()
        .expect("load stored credentials")
        .expect("stored token set");
    assert_eq!(stored.refresh_token.as_deref(), Some("refresh-token"));

    clear_oauth_credentials().expect("clear credentials");
    std::env::remove_var("CODINEER_CONFIG_HOME");
    cleanup_temp_config_home(&config_home);
}

#[test]
fn message_request_stream_helper_sets_stream_true() {
    let request = MessageRequest {
        model: "claude-opus-4-6".to_string(),
        max_tokens: 64,
        messages: vec![],
        system: None,
        tools: None,
        tool_choice: None,
        stream: false,
    };

    assert!(request.with_streaming().stream);
}

#[test]
fn backoff_doubles_until_maximum() {
    let client = CodineerApiClient::new("test-key").with_retry_policy(RetryPolicy {
        max_retries: 3,
        initial_backoff: Duration::from_millis(10),
        max_backoff: Duration::from_millis(25),
    });
    assert_eq!(
        client.backoff_for_attempt(1).expect("attempt 1"),
        Duration::from_millis(10)
    );
    assert_eq!(
        client.backoff_for_attempt(2).expect("attempt 2"),
        Duration::from_millis(20)
    );
    assert_eq!(
        client.backoff_for_attempt(3).expect("attempt 3"),
        Duration::from_millis(25)
    );
}

#[test]
fn retryable_statuses_are_detected() {
    assert!(super::is_retryable_status(
        reqwest::StatusCode::TOO_MANY_REQUESTS
    ));
    assert!(super::is_retryable_status(
        reqwest::StatusCode::INTERNAL_SERVER_ERROR
    ));
    assert!(!super::is_retryable_status(
        reqwest::StatusCode::UNAUTHORIZED
    ));
}

#[test]
fn tool_delta_variant_round_trips() {
    let delta = ContentBlockDelta::InputJsonDelta {
        partial_json: "{\"city\":\"Paris\"}".to_string(),
    };
    let encoded = serde_json::to_string(&delta).expect("delta should serialize");
    let decoded: ContentBlockDelta =
        serde_json::from_str(&encoded).expect("delta should deserialize");
    assert_eq!(decoded, delta);
}

#[test]
fn request_id_uses_primary_or_fallback_header() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(REQUEST_ID_HEADER, "req_primary".parse().expect("header"));
    assert_eq!(
        super::request_id_from_headers(&headers).as_deref(),
        Some("req_primary")
    );

    headers.clear();
    headers.insert(
        ALT_REQUEST_ID_HEADER,
        "req_fallback".parse().expect("header"),
    );
    assert_eq!(
        super::request_id_from_headers(&headers).as_deref(),
        Some("req_fallback")
    );
}

#[test]
fn auth_source_applies_headers() {
    let auth = AuthSource::ApiKeyAndBearer {
        api_key: "test-key".to_string(),
        bearer_token: "proxy-token".to_string(),
    };
    let request = auth
        .apply(reqwest::Client::new().post("https://example.test"))
        .build()
        .expect("request build");
    let headers = request.headers();
    assert_eq!(
        headers.get("x-api-key").and_then(|v| v.to_str().ok()),
        Some("test-key")
    );
    assert_eq!(
        headers.get("authorization").and_then(|v| v.to_str().ok()),
        Some("Bearer proxy-token")
    );
}

#[test]
fn auth_source_debug_redacts_secrets() {
    let key = AuthSource::ApiKey("sk-secret-key-12345".to_string());
    let debug = format!("{key:?}");
    assert!(
        !debug.contains("sk-secret"),
        "Debug must not leak API key: {debug}"
    );
    assert!(
        debug.contains("***"),
        "Debug should show redacted marker: {debug}"
    );

    let bearer = AuthSource::BearerToken("bearer-secret-token".to_string());
    let debug = format!("{bearer:?}");
    assert!(
        !debug.contains("bearer-secret"),
        "Debug must not leak bearer token: {debug}"
    );

    let both = AuthSource::ApiKeyAndBearer {
        api_key: "key".to_string(),
        bearer_token: "tok".to_string(),
    };
    let debug = format!("{both:?}");
    assert!(!debug.contains("key"), "Debug must not leak keys: {debug}");
    assert!(
        !debug.contains("tok"),
        "Debug must not leak tokens: {debug}"
    );
}

// -----------------------------------------------------------------------
// From<ResolvedCredential> for AuthSource
// -----------------------------------------------------------------------

#[test]
fn resolved_credential_api_key_maps_to_auth_source_api_key() {
    let cred = runtime::ResolvedCredential::ApiKey("sk-key".into());
    let auth = AuthSource::from(cred);
    assert_eq!(auth.api_key(), Some("sk-key"));
    assert_eq!(auth.bearer_token(), None);
}

#[test]
fn resolved_credential_bearer_maps_to_auth_source_bearer() {
    let cred = runtime::ResolvedCredential::BearerToken("tok-123".into());
    let auth = AuthSource::from(cred);
    assert_eq!(auth.bearer_token(), Some("tok-123"));
    assert_eq!(auth.api_key(), None);
}

#[test]
fn resolved_credential_both_maps_to_auth_source_both() {
    let cred = runtime::ResolvedCredential::ApiKeyAndBearer {
        api_key: "key".into(),
        bearer_token: "tok".into(),
    };
    let auth = AuthSource::from(cred);
    assert_eq!(auth.api_key(), Some("key"));
    assert_eq!(auth.bearer_token(), Some("tok"));
}
