use super::{
    ConfigLoader, ConfigSource, McpServerConfig, McpTransport, ResolvedPermissionMode,
    CODINEER_SETTINGS_SCHEMA_NAME,
};
use crate::json::JsonValue;
use crate::sandbox::FilesystemIsolationMode;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> std::path::PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("runtime-config-{nanos}-{count}"))
}

#[test]
fn rejects_non_object_settings_files() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(&home).expect("home config dir");
    fs::create_dir_all(&cwd).expect("project dir");
    fs::write(home.join("settings.json"), "[]").expect("write bad settings");

    let error = ConfigLoader::new(&cwd, &home)
        .load()
        .expect_err("config should fail");
    assert!(error
        .to_string()
        .contains("top-level settings value must be a JSON object"));

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn loads_and_merges_config_files_by_precedence() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(cwd.join(".codineer")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");

    fs::write(
        home.join("settings.json"),
        r#"{"model":"sonnet","env":{"A":"1","A2":"1"},"hooks":{"PreToolUse":["base"]},"permissions":{"defaultMode":"plan"},"mcpServers":{"home":{"command":"uvx","args":["home"]}}}"#,
    )
    .expect("write user settings");
    fs::write(
        cwd.join(".codineer").join("settings.json"),
        r#"{"env":{"B":"2","C":"3"},"hooks":{"PostToolUse":["project"]},"mcpServers":{"project":{"command":"uvx","args":["project"]}}}"#,
    )
    .expect("write project settings");
    fs::write(
        cwd.join(".codineer").join("settings.local.json"),
        r#"{"model":"opus","permissionMode":"acceptEdits"}"#,
    )
    .expect("write local settings");

    let loaded = ConfigLoader::new(&cwd, &home)
        .load()
        .expect("config should load");

    assert_eq!(CODINEER_SETTINGS_SCHEMA_NAME, "SettingsSchema");
    assert_eq!(loaded.loaded_entries().len(), 3);
    assert_eq!(loaded.loaded_entries()[0].source, ConfigSource::User);
    assert_eq!(
        loaded.get("model"),
        Some(&JsonValue::String("opus".to_string()))
    );
    assert_eq!(loaded.model(), Some("opus"));
    assert_eq!(
        loaded.permission_mode(),
        Some(ResolvedPermissionMode::WorkspaceWrite)
    );
    assert_eq!(
        loaded
            .get("env")
            .and_then(JsonValue::as_object)
            .expect("env object")
            .len(),
        4
    );
    assert!(loaded
        .get("hooks")
        .and_then(JsonValue::as_object)
        .expect("hooks object")
        .contains_key("PreToolUse"));
    assert!(loaded
        .get("hooks")
        .and_then(JsonValue::as_object)
        .expect("hooks object")
        .contains_key("PostToolUse"));
    assert_eq!(loaded.hooks().pre_tool_use(), &["base".to_string()]);
    assert_eq!(loaded.hooks().post_tool_use(), &["project".to_string()]);
    assert!(loaded.mcp().get("home").is_some());
    assert!(loaded.mcp().get("project").is_some());

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn parses_sandbox_config() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(cwd.join(".codineer")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");

    fs::write(
        cwd.join(".codineer").join("settings.local.json"),
        r#"{
          "sandbox": {
            "enabled": true,
            "namespaceRestrictions": false,
            "networkIsolation": true,
            "filesystemMode": "allow-list",
            "allowedMounts": ["logs", "tmp/cache"]
          }
        }"#,
    )
    .expect("write local settings");

    let loaded = ConfigLoader::new(&cwd, &home)
        .load()
        .expect("config should load");

    assert_eq!(loaded.sandbox().enabled, Some(true));
    assert_eq!(loaded.sandbox().namespace_restrictions, Some(false));
    assert_eq!(loaded.sandbox().network_isolation, Some(true));
    assert_eq!(
        loaded.sandbox().filesystem_mode,
        Some(FilesystemIsolationMode::AllowList)
    );
    assert_eq!(loaded.sandbox().allowed_mounts, vec!["logs", "tmp/cache"]);

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn parses_typed_mcp_and_oauth_config() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(cwd.join(".codineer")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");

    fs::write(
        home.join("settings.json"),
        r#"{
          "mcpServers": {
            "stdio-server": {
              "command": "uvx",
              "args": ["mcp-server"],
              "env": {"TOKEN": "secret"}
            },
            "remote-server": {
              "type": "http",
              "url": "https://example.test/mcp",
              "headers": {"Authorization": "Bearer token"},
              "headersHelper": "helper.sh",
              "oauth": {
                "clientId": "mcp-client",
                "callbackPort": 7777,
                "authServerMetadataUrl": "https://issuer.test/.well-known/oauth-authorization-server",
                "xaa": true
              }
            }
          },
          "oauth": {
            "clientId": "runtime-client",
            "authorizeUrl": "https://console.test/oauth/authorize",
            "tokenUrl": "https://console.test/oauth/token",
            "callbackPort": 54545,
            "manualRedirectUrl": "https://console.test/oauth/callback",
            "scopes": ["org:read", "user:write"]
          }
        }"#,
    )
    .expect("write user settings");
    fs::write(
        cwd.join(".codineer").join("settings.local.json"),
        r#"{
          "mcpServers": {
            "remote-server": {
              "type": "ws",
              "url": "wss://override.test/mcp",
              "headers": {"X-Env": "local"}
            }
          }
        }"#,
    )
    .expect("write local settings");

    let loaded = ConfigLoader::new(&cwd, &home)
        .load()
        .expect("config should load");

    let stdio_server = loaded
        .mcp()
        .get("stdio-server")
        .expect("stdio server should exist");
    assert_eq!(stdio_server.scope, ConfigSource::User);
    assert_eq!(stdio_server.transport(), McpTransport::Stdio);

    let remote_server = loaded
        .mcp()
        .get("remote-server")
        .expect("remote server should exist");
    assert_eq!(remote_server.scope, ConfigSource::Local);
    assert_eq!(remote_server.transport(), McpTransport::Ws);
    match &remote_server.config {
        McpServerConfig::Ws(config) => {
            assert_eq!(config.url, "wss://override.test/mcp");
            assert_eq!(
                config.headers.get("X-Env").map(String::as_str),
                Some("local")
            );
        }
        other => panic!("expected ws config, got {other:?}"),
    }

    let oauth = loaded.oauth().expect("oauth config should exist");
    assert_eq!(oauth.client_id, "runtime-client");
    assert_eq!(oauth.callback_port, Some(54_545));
    assert_eq!(oauth.scopes, vec!["org:read", "user:write"]);

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn parses_plugin_config_from_enabled_plugins() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(cwd.join(".codineer")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");

    fs::write(
        home.join("settings.json"),
        r#"{
          "enabledPlugins": {
            "tool-guard@builtin": true,
            "sample-plugin@external": false
          }
        }"#,
    )
    .expect("write user settings");

    let loaded = ConfigLoader::new(&cwd, &home)
        .load()
        .expect("config should load");

    assert_eq!(
        loaded.plugins().enabled_plugins().get("tool-guard@builtin"),
        Some(&true)
    );
    assert_eq!(
        loaded
            .plugins()
            .enabled_plugins()
            .get("sample-plugin@external"),
        Some(&false)
    );

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn parses_plugin_config() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(cwd.join(".codineer")).expect("project config dir");
    fs::create_dir_all(&home).expect("home config dir");

    fs::write(
        home.join("settings.json"),
        r#"{
          "enabledPlugins": {
            "core-helpers@builtin": true
          },
          "plugins": {
            "externalDirectories": ["./external-plugins"],
            "installRoot": "plugin-cache/installed",
            "registryPath": "plugin-cache/installed.json",
            "bundledRoot": "./bundled-plugins"
          }
        }"#,
    )
    .expect("write plugin settings");

    let loaded = ConfigLoader::new(&cwd, &home)
        .load()
        .expect("config should load");

    assert_eq!(
        loaded
            .plugins()
            .enabled_plugins()
            .get("core-helpers@builtin"),
        Some(&true)
    );
    assert_eq!(
        loaded.plugins().external_directories(),
        &["./external-plugins".to_string()]
    );
    assert_eq!(
        loaded.plugins().install_root(),
        Some("plugin-cache/installed")
    );
    assert_eq!(
        loaded.plugins().registry_path(),
        Some("plugin-cache/installed.json")
    );
    assert_eq!(loaded.plugins().bundled_root(), Some("./bundled-plugins"));

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn rejects_invalid_mcp_server_shapes() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(&home).expect("home config dir");
    fs::create_dir_all(&cwd).expect("project dir");
    fs::write(
        home.join("settings.json"),
        r#"{"mcpServers":{"broken":{"type":"http","url":123}}}"#,
    )
    .expect("write broken settings");

    let error = ConfigLoader::new(&cwd, &home)
        .load()
        .expect_err("config should fail");
    assert!(error
        .to_string()
        .contains("mcpServers.broken: missing string field url"));

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn parses_sse_and_sdk_and_managed_proxy_mcp_types() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(&home).expect("home dir");
    fs::create_dir_all(&cwd).expect("project dir");
    fs::write(
        home.join("settings.json"),
        r#"{
            "mcpServers": {
                "events": {"type": "sse", "url": "https://sse.example/events"},
                "built-in": {"type": "sdk", "name": "sdk-server"},
                "proxy": {"type": "claudeai-proxy", "url": "https://proxy.example", "id": "p1"}
            }
        }"#,
    )
    .expect("write mcp settings");

    let loaded = ConfigLoader::new(&cwd, &home).load().expect("load config");
    let servers = loaded.mcp().servers();
    assert_eq!(servers.len(), 3);
    assert!(matches!(&servers["events"].config, McpServerConfig::Sse(_)));
    assert!(matches!(
        &servers["built-in"].config,
        McpServerConfig::Sdk(_)
    ));
    assert!(matches!(
        &servers["proxy"].config,
        McpServerConfig::ManagedProxy(_)
    ));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn rejects_unsupported_mcp_server_type() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(&home).expect("home dir");
    fs::create_dir_all(&cwd).expect("project dir");
    fs::write(
        home.join("settings.json"),
        r#"{"mcpServers":{"x":{"type":"grpc","url":"x"}}}"#,
    )
    .expect("write");

    let err = ConfigLoader::new(&cwd, &home)
        .load()
        .expect_err("should fail");
    assert!(err.to_string().contains("unsupported MCP server type"));

    fs::remove_dir_all(root).expect("cleanup");
}


#[test]
fn parses_permission_and_sandbox_mode_labels() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(&home).expect("home dir");
    fs::create_dir_all(&cwd).expect("project dir");
    fs::write(
        home.join("settings.json"),
        r#"{"permissionMode":"read-only","sandbox":{"filesystemMode":"off"}}"#,
    )
    .expect("write settings");

    let loaded = ConfigLoader::new(&cwd, &home).load().expect("load");
    assert!(matches!(
        loaded.permission_mode(),
        Some(ResolvedPermissionMode::ReadOnly)
    ));
    let sandbox = loaded.sandbox();
    assert_eq!(
        sandbox.filesystem_mode,
        Some(crate::sandbox::FilesystemIsolationMode::Off)
    );

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn parses_credentials_config() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(&home).expect("home dir");
    fs::create_dir_all(&cwd).expect("project dir");
    fs::write(
        home.join("settings.json"),
        r#"{"credentials":{"defaultSource":"codineer-oauth","autoDiscover":true,"claudeCode":{"enabled":false}}}"#,
    )
    .expect("write settings");

    let loaded = ConfigLoader::new(&cwd, &home).load().expect("load");
    let cred = loaded.credentials();
    assert_eq!(cred.default_source.as_deref(), Some("codineer-oauth"));
    assert!(cred.auto_discover);
    assert!(!cred.claude_code_enabled);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn credentials_config_defaults_when_absent() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(&home).expect("home dir");
    fs::create_dir_all(&cwd).expect("project dir");
    fs::write(home.join("settings.json"), r#"{}"#).expect("write settings");

    let loaded = ConfigLoader::new(&cwd, &home).load().expect("load");
    let cred = loaded.credentials();
    assert!(cred.default_source.is_none());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn parses_fallback_models_from_settings() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(&home).expect("home dir");
    fs::create_dir_all(&cwd).expect("project dir");
    fs::write(
        home.join("settings.json"),
        r#"{"fallbackModels":["ollama/qwen3-coder","groq/llama-3.3-70b-versatile"]}"#,
    )
    .expect("write settings");

    let loaded = ConfigLoader::new(&cwd, &home).load().expect("load");
    assert_eq!(
        loaded.fallback_models(),
        &["ollama/qwen3-coder", "groq/llama-3.3-70b-versatile"]
    );

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn fallback_models_defaults_to_empty_when_absent() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(&home).expect("home dir");
    fs::create_dir_all(&cwd).expect("project dir");
    fs::write(home.join("settings.json"), r#"{"model":"sonnet"}"#).expect("write settings");

    let loaded = ConfigLoader::new(&cwd, &home).load().expect("load");
    assert!(loaded.fallback_models().is_empty());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn fallback_models_ignores_non_string_elements() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(&home).expect("home dir");
    fs::create_dir_all(&cwd).expect("project dir");
    fs::write(
        home.join("settings.json"),
        r#"{"fallbackModels":["valid", 42, null, "also-valid"]}"#,
    )
    .expect("write settings");

    let loaded = ConfigLoader::new(&cwd, &home).load().expect("load");
    assert_eq!(loaded.fallback_models(), &["valid", "also-valid"]);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn fallback_models_empty_array_returns_empty() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(&home).expect("home dir");
    fs::create_dir_all(&cwd).expect("project dir");
    fs::write(home.join("settings.json"), r#"{"fallbackModels":[]}"#).expect("write settings");

    let loaded = ConfigLoader::new(&cwd, &home).load().expect("load");
    assert!(loaded.fallback_models().is_empty());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn fallback_models_non_array_value_returns_empty() {
    let root = temp_dir();
    let cwd = root.join("project");
    let home = root.join("home").join(".codineer");
    fs::create_dir_all(&home).expect("home dir");
    fs::create_dir_all(&cwd).expect("project dir");
    fs::write(
        home.join("settings.json"),
        r#"{"fallbackModels":"not-an-array"}"#,
    )
    .expect("write settings");

    let loaded = ConfigLoader::new(&cwd, &home).load().expect("load");
    assert!(loaded.fallback_models().is_empty());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn config_source_display_and_as_str() {
    assert_eq!(ConfigSource::User.as_str(), "user");
    assert_eq!(ConfigSource::Project.as_str(), "project");
    assert_eq!(ConfigSource::Local.as_str(), "local");
    assert_eq!(ConfigSource::User.to_string(), "user");
    assert_eq!(ConfigSource::Project.to_string(), "project");
    assert_eq!(ConfigSource::Local.to_string(), "local");
}
