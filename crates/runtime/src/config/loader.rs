use std::borrow::ToOwned;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::json::JsonValue;
use crate::sandbox::{FilesystemIsolationMode, SandboxConfig};

use super::types::*;

#[must_use]
pub fn default_config_home() -> PathBuf {
    std::env::var_os("CODINEER_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| crate::home_dir().map(|home| home.join(".codineer")))
        .unwrap_or_else(|| PathBuf::from(".codineer"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigLoader {
    cwd: PathBuf,
    config_home: PathBuf,
}

impl ConfigLoader {
    #[must_use]
    pub fn new(cwd: impl Into<PathBuf>, config_home: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            config_home: config_home.into(),
        }
    }

    #[must_use]
    pub fn default_for(cwd: impl Into<PathBuf>) -> Self {
        let cwd = cwd.into();
        let config_home = default_config_home();
        Self { cwd, config_home }
    }

    #[must_use]
    pub fn config_home(&self) -> &Path {
        &self.config_home
    }

    #[must_use]
    pub fn discover(&self) -> Vec<ConfigEntry> {
        let user_flat_config = self.config_home.parent().map_or_else(
            || PathBuf::from(".codineer.json"),
            |parent| parent.join(".codineer.json"),
        );
        vec![
            ConfigEntry {
                source: ConfigSource::User,
                path: user_flat_config,
            },
            ConfigEntry {
                source: ConfigSource::User,
                path: self.config_home.join("settings.json"),
            },
            ConfigEntry {
                source: ConfigSource::Project,
                path: self.cwd.join(".codineer.json"),
            },
            ConfigEntry {
                source: ConfigSource::Project,
                path: self.cwd.join(".codineer").join("settings.json"),
            },
            ConfigEntry {
                source: ConfigSource::Local,
                path: self.cwd.join(".codineer").join("settings.local.json"),
            },
        ]
    }

    pub fn load(&self) -> Result<RuntimeConfig, ConfigError> {
        let mut merged = BTreeMap::new();
        let mut loaded_entries = Vec::new();
        let mut mcp_servers = BTreeMap::new();

        let mut config_warnings = Vec::new();
        for entry in self.discover() {
            let Some(value) = read_optional_json_object(&entry.path, &mut config_warnings)? else {
                continue;
            };
            merge_mcp_servers(&mut mcp_servers, entry.source, &value, &entry.path)?;
            deep_merge_objects(&mut merged, &value);
            loaded_entries.push(entry);
        }

        let merged_value = JsonValue::Object(merged.clone());

        let feature_config = RuntimeFeatureConfig {
            hooks: parse_optional_hooks_config(&merged_value)?,
            plugins: parse_optional_plugin_config(&merged_value)?,
            mcp: McpConfigCollection {
                servers: mcp_servers,
            },
            oauth: parse_optional_oauth_config(&merged_value, "merged settings.oauth")?,
            model: parse_optional_model(&merged_value),
            fallback_models: parse_optional_fallback_models(&merged_value),
            permission_mode: parse_optional_permission_mode(&merged_value)?,
            sandbox: parse_optional_sandbox_config(&merged_value)?,
            providers: parse_optional_providers_config(&merged_value)?,
            credentials: parse_optional_credentials_config(&merged_value)?,
        };

        for w in &config_warnings {
            eprintln!("warning: {w}");
        }

        Ok(RuntimeConfig::new(merged, loaded_entries, feature_config))
    }
}

fn read_optional_json_object(
    path: &Path,
    warnings: &mut Vec<String>,
) -> Result<Option<BTreeMap<String, JsonValue>>, ConfigError> {
    let is_flat_config = path.file_name().and_then(|name| name.to_str()) == Some(".codineer.json");
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ConfigError::Io(error)),
    };

    if contents.trim().is_empty() {
        return Ok(Some(BTreeMap::new()));
    }

    let parsed = match JsonValue::parse(&contents) {
        Ok(parsed) => parsed,
        Err(error) if is_flat_config => {
            warnings.push(format!(
                "ignoring malformed config '{}': {error}",
                path.display()
            ));
            return Ok(None);
        }
        Err(error) => return Err(ConfigError::Parse(format!("{}: {error}", path.display()))),
    };
    let Some(object) = parsed.as_object() else {
        if is_flat_config {
            warnings.push(format!(
                "ignoring config '{}': expected JSON object at top level",
                path.display()
            ));
            return Ok(None);
        }
        return Err(ConfigError::Parse(format!(
            "{}: top-level settings value must be a JSON object",
            path.display()
        )));
    };
    Ok(Some(object.clone()))
}

fn merge_mcp_servers(
    target: &mut BTreeMap<String, ScopedMcpServerConfig>,
    source: ConfigSource,
    root: &BTreeMap<String, JsonValue>,
    path: &Path,
) -> Result<(), ConfigError> {
    let Some(mcp_servers) = root.get("mcpServers") else {
        return Ok(());
    };
    let servers = expect_object(mcp_servers, &format!("{}: mcpServers", path.display()))?;
    for (name, value) in servers {
        let parsed = parse_mcp_server_config(
            name,
            value,
            &format!("{}: mcpServers.{name}", path.display()),
        )?;
        target.insert(
            name.clone(),
            ScopedMcpServerConfig {
                scope: source,
                config: parsed,
            },
        );
    }
    Ok(())
}

fn parse_optional_model(root: &JsonValue) -> Option<String> {
    root.as_object()
        .and_then(|object| object.get("model"))
        .and_then(JsonValue::as_str)
        .map(ToOwned::to_owned)
}

fn parse_optional_fallback_models(root: &JsonValue) -> Vec<String> {
    root.as_object()
        .and_then(|object| object.get("fallbackModels"))
        .and_then(JsonValue::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(JsonValue::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_optional_providers_config(
    root: &JsonValue,
) -> Result<BTreeMap<String, CustomProviderConfig>, ConfigError> {
    let Some(object) = root.as_object() else {
        return Ok(BTreeMap::new());
    };
    let Some(providers_value) = object.get("providers") else {
        return Ok(BTreeMap::new());
    };
    let providers_obj = expect_object(providers_value, "merged settings.providers")?;
    let mut result = BTreeMap::new();
    for (name, value) in providers_obj {
        let ctx = format!("merged settings.providers.{name}");
        let provider_obj = expect_object(value, &ctx)?;
        let base_url = expect_string(provider_obj, "baseUrl", &ctx)?.to_string();
        let api_key = optional_string(provider_obj, "apiKey", &ctx)?.map(str::to_string);
        let api_key_env = optional_string(provider_obj, "apiKeyEnv", &ctx)?.map(str::to_string);
        let models = optional_string_array(provider_obj, "models", &ctx)?.unwrap_or_default();
        let default_model =
            optional_string(provider_obj, "defaultModel", &ctx)?.map(str::to_string);
        result.insert(
            name.clone(),
            CustomProviderConfig {
                base_url,
                api_key,
                api_key_env,
                models,
                default_model,
            },
        );
    }
    Ok(result)
}

fn parse_optional_hooks_config(root: &JsonValue) -> Result<RuntimeHookConfig, ConfigError> {
    let Some(object) = root.as_object() else {
        return Ok(RuntimeHookConfig::default());
    };
    let Some(hooks_value) = object.get("hooks") else {
        return Ok(RuntimeHookConfig::default());
    };
    let hooks = expect_object(hooks_value, "merged settings.hooks")?;
    Ok(RuntimeHookConfig {
        pre_tool_use: optional_string_array(hooks, "PreToolUse", "merged settings.hooks")?
            .unwrap_or_default(),
        post_tool_use: optional_string_array(hooks, "PostToolUse", "merged settings.hooks")?
            .unwrap_or_default(),
    })
}

fn parse_optional_plugin_config(root: &JsonValue) -> Result<RuntimePluginConfig, ConfigError> {
    let Some(object) = root.as_object() else {
        return Ok(RuntimePluginConfig::default());
    };

    let mut config = RuntimePluginConfig::default();
    if let Some(enabled_plugins) = object.get("enabledPlugins") {
        config.enabled_plugins = parse_bool_map(enabled_plugins, "merged settings.enabledPlugins")?;
    }

    let Some(plugins_value) = object.get("plugins") else {
        return Ok(config);
    };
    let plugins = expect_object(plugins_value, "merged settings.plugins")?;

    if let Some(enabled_value) = plugins.get("enabled") {
        config.enabled_plugins = parse_bool_map(enabled_value, "merged settings.plugins.enabled")?;
    }
    config.external_directories =
        optional_string_array(plugins, "externalDirectories", "merged settings.plugins")?
            .unwrap_or_default();
    config.install_root =
        optional_string(plugins, "installRoot", "merged settings.plugins")?.map(str::to_string);
    config.registry_path =
        optional_string(plugins, "registryPath", "merged settings.plugins")?.map(str::to_string);
    config.bundled_root =
        optional_string(plugins, "bundledRoot", "merged settings.plugins")?.map(str::to_string);
    Ok(config)
}

fn parse_optional_permission_mode(
    root: &JsonValue,
) -> Result<Option<ResolvedPermissionMode>, ConfigError> {
    let Some(object) = root.as_object() else {
        return Ok(None);
    };
    if let Some(mode) = object.get("permissionMode").and_then(JsonValue::as_str) {
        return parse_permission_mode_label(mode, "merged settings.permissionMode").map(Some);
    }
    let Some(mode) = object
        .get("permissions")
        .and_then(JsonValue::as_object)
        .and_then(|permissions| permissions.get("defaultMode"))
        .and_then(JsonValue::as_str)
    else {
        return Ok(None);
    };
    parse_permission_mode_label(mode, "merged settings.permissions.defaultMode").map(Some)
}

fn parse_permission_mode_label(
    mode: &str,
    context: &str,
) -> Result<ResolvedPermissionMode, ConfigError> {
    match mode {
        "default" | "plan" | "read-only" => Ok(ResolvedPermissionMode::ReadOnly),
        "acceptEdits" | "auto" | "workspace-write" => Ok(ResolvedPermissionMode::WorkspaceWrite),
        "dontAsk" | "danger-full-access" => Ok(ResolvedPermissionMode::DangerFullAccess),
        other => Err(ConfigError::Parse(format!(
            "{context}: unsupported permission mode {other}"
        ))),
    }
}

fn parse_optional_sandbox_config(root: &JsonValue) -> Result<SandboxConfig, ConfigError> {
    let Some(object) = root.as_object() else {
        return Ok(SandboxConfig::default());
    };
    let Some(sandbox_value) = object.get("sandbox") else {
        return Ok(SandboxConfig::default());
    };
    let sandbox = expect_object(sandbox_value, "merged settings.sandbox")?;
    let filesystem_mode = optional_string(sandbox, "filesystemMode", "merged settings.sandbox")?
        .map(parse_filesystem_mode_label)
        .transpose()?;
    Ok(SandboxConfig {
        enabled: optional_bool(sandbox, "enabled", "merged settings.sandbox")?,
        namespace_restrictions: optional_bool(
            sandbox,
            "namespaceRestrictions",
            "merged settings.sandbox",
        )?,
        network_isolation: optional_bool(sandbox, "networkIsolation", "merged settings.sandbox")?,
        filesystem_mode,
        allowed_mounts: optional_string_array(sandbox, "allowedMounts", "merged settings.sandbox")?
            .unwrap_or_default(),
    })
}

fn parse_filesystem_mode_label(value: &str) -> Result<FilesystemIsolationMode, ConfigError> {
    match value {
        "off" => Ok(FilesystemIsolationMode::Off),
        "workspace-only" => Ok(FilesystemIsolationMode::WorkspaceOnly),
        "allow-list" => Ok(FilesystemIsolationMode::AllowList),
        other => Err(ConfigError::Parse(format!(
            "merged settings.sandbox.filesystemMode: unsupported filesystem mode {other}"
        ))),
    }
}

fn parse_optional_oauth_config(
    root: &JsonValue,
    context: &str,
) -> Result<Option<OAuthConfig>, ConfigError> {
    let Some(oauth_value) = root.as_object().and_then(|object| object.get("oauth")) else {
        return Ok(None);
    };
    let object = expect_object(oauth_value, context)?;
    let client_id = expect_string(object, "clientId", context)?.to_string();
    let authorize_url = expect_string(object, "authorizeUrl", context)?.to_string();
    let token_url = expect_string(object, "tokenUrl", context)?.to_string();
    let callback_port = optional_u16(object, "callbackPort", context)?;
    let manual_redirect_url =
        optional_string(object, "manualRedirectUrl", context)?.map(str::to_string);
    let scopes = optional_string_array(object, "scopes", context)?.unwrap_or_default();
    Ok(Some(OAuthConfig {
        client_id,
        authorize_url,
        token_url,
        callback_port,
        manual_redirect_url,
        scopes,
    }))
}

fn parse_mcp_server_config(
    server_name: &str,
    value: &JsonValue,
    context: &str,
) -> Result<McpServerConfig, ConfigError> {
    let object = expect_object(value, context)?;
    let server_type = optional_string(object, "type", context)?.unwrap_or("stdio");
    match server_type {
        "stdio" => Ok(McpServerConfig::Stdio(McpStdioServerConfig {
            command: expect_string(object, "command", context)?.to_string(),
            args: optional_string_array(object, "args", context)?.unwrap_or_default(),
            env: optional_string_map(object, "env", context)?.unwrap_or_default(),
        })),
        "sse" => Ok(McpServerConfig::Sse(parse_mcp_remote_server_config(
            object, context,
        )?)),
        "http" => Ok(McpServerConfig::Http(parse_mcp_remote_server_config(
            object, context,
        )?)),
        "ws" | "websocket" => Ok(McpServerConfig::Ws(McpWebSocketServerConfig {
            url: expect_string(object, "url", context)?.to_string(),
            headers: optional_string_map(object, "headers", context)?.unwrap_or_default(),
            headers_helper: optional_string(object, "headersHelper", context)?.map(str::to_string),
        })),
        "sdk" => Ok(McpServerConfig::Sdk(McpSdkServerConfig {
            name: expect_string(object, "name", context)?.to_string(),
        })),
        "claudeai-proxy" => Ok(McpServerConfig::ManagedProxy(McpManagedProxyServerConfig {
            url: expect_string(object, "url", context)?.to_string(),
            id: expect_string(object, "id", context)?.to_string(),
        })),
        other => Err(ConfigError::Parse(format!(
            "{context}: unsupported MCP server type for {server_name}: {other}"
        ))),
    }
}

fn parse_mcp_remote_server_config(
    object: &BTreeMap<String, JsonValue>,
    context: &str,
) -> Result<McpRemoteServerConfig, ConfigError> {
    Ok(McpRemoteServerConfig {
        url: expect_string(object, "url", context)?.to_string(),
        headers: optional_string_map(object, "headers", context)?.unwrap_or_default(),
        headers_helper: optional_string(object, "headersHelper", context)?.map(str::to_string),
        oauth: parse_optional_mcp_oauth_config(object, context)?,
    })
}

fn parse_optional_mcp_oauth_config(
    object: &BTreeMap<String, JsonValue>,
    context: &str,
) -> Result<Option<McpOAuthConfig>, ConfigError> {
    let Some(value) = object.get("oauth") else {
        return Ok(None);
    };
    let oauth = expect_object(value, &format!("{context}.oauth"))?;
    Ok(Some(McpOAuthConfig {
        client_id: optional_string(oauth, "clientId", context)?.map(str::to_string),
        callback_port: optional_u16(oauth, "callbackPort", context)?,
        auth_server_metadata_url: optional_string(oauth, "authServerMetadataUrl", context)?
            .map(str::to_string),
        xaa: optional_bool(oauth, "xaa", context)?,
    }))
}

fn expect_object<'a>(
    value: &'a JsonValue,
    context: &str,
) -> Result<&'a BTreeMap<String, JsonValue>, ConfigError> {
    value
        .as_object()
        .ok_or_else(|| ConfigError::Parse(format!("{context}: expected JSON object")))
}

fn expect_string<'a>(
    object: &'a BTreeMap<String, JsonValue>,
    key: &str,
    context: &str,
) -> Result<&'a str, ConfigError> {
    object
        .get(key)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| ConfigError::Parse(format!("{context}: missing string field {key}")))
}

fn optional_string<'a>(
    object: &'a BTreeMap<String, JsonValue>,
    key: &str,
    context: &str,
) -> Result<Option<&'a str>, ConfigError> {
    match object.get(key) {
        Some(value) => value
            .as_str()
            .map(Some)
            .ok_or_else(|| ConfigError::Parse(format!("{context}: field {key} must be a string"))),
        None => Ok(None),
    }
}

fn optional_bool(
    object: &BTreeMap<String, JsonValue>,
    key: &str,
    context: &str,
) -> Result<Option<bool>, ConfigError> {
    match object.get(key) {
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or_else(|| ConfigError::Parse(format!("{context}: field {key} must be a boolean"))),
        None => Ok(None),
    }
}

fn optional_u16(
    object: &BTreeMap<String, JsonValue>,
    key: &str,
    context: &str,
) -> Result<Option<u16>, ConfigError> {
    match object.get(key) {
        Some(value) => {
            let Some(number) = value.as_i64() else {
                return Err(ConfigError::Parse(format!(
                    "{context}: field {key} must be an integer"
                )));
            };
            let number = u16::try_from(number).map_err(|_| {
                ConfigError::Parse(format!("{context}: field {key} is out of range"))
            })?;
            Ok(Some(number))
        }
        None => Ok(None),
    }
}

fn parse_bool_map(value: &JsonValue, context: &str) -> Result<BTreeMap<String, bool>, ConfigError> {
    let Some(map) = value.as_object() else {
        return Err(ConfigError::Parse(format!(
            "{context}: expected JSON object"
        )));
    };
    map.iter()
        .map(|(key, value)| {
            value
                .as_bool()
                .map(|enabled| (key.clone(), enabled))
                .ok_or_else(|| {
                    ConfigError::Parse(format!("{context}: field {key} must be a boolean"))
                })
        })
        .collect()
}

fn optional_string_array(
    object: &BTreeMap<String, JsonValue>,
    key: &str,
    context: &str,
) -> Result<Option<Vec<String>>, ConfigError> {
    match object.get(key) {
        Some(value) => {
            let Some(array) = value.as_array() else {
                return Err(ConfigError::Parse(format!(
                    "{context}: field {key} must be an array"
                )));
            };
            array
                .iter()
                .map(|item| {
                    item.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                        ConfigError::Parse(format!(
                            "{context}: field {key} must contain only strings"
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Some)
        }
        None => Ok(None),
    }
}

fn optional_string_map(
    object: &BTreeMap<String, JsonValue>,
    key: &str,
    context: &str,
) -> Result<Option<BTreeMap<String, String>>, ConfigError> {
    match object.get(key) {
        Some(value) => {
            let Some(map) = value.as_object() else {
                return Err(ConfigError::Parse(format!(
                    "{context}: field {key} must be an object"
                )));
            };
            map.iter()
                .map(|(entry_key, entry_value)| {
                    entry_value
                        .as_str()
                        .map(|text| (entry_key.clone(), text.to_string()))
                        .ok_or_else(|| {
                            ConfigError::Parse(format!(
                                "{context}: field {key} must contain only string values"
                            ))
                        })
                })
                .collect::<Result<BTreeMap<_, _>, _>>()
                .map(Some)
        }
        None => Ok(None),
    }
}

fn parse_optional_credentials_config(root: &JsonValue) -> Result<CredentialConfig, ConfigError> {
    let Some(object) = root.as_object() else {
        return Ok(CredentialConfig::default());
    };
    let Some(cred_value) = object.get("credentials") else {
        return Ok(CredentialConfig::default());
    };
    let cred = expect_object(cred_value, "merged settings.credentials")?;

    let default_source =
        optional_string(cred, "defaultSource", "merged settings.credentials")?.map(str::to_string);
    let auto_discover =
        optional_bool(cred, "autoDiscover", "merged settings.credentials")?.unwrap_or(true);

    let claude_code_enabled = cred
        .get("claudeCode")
        .and_then(JsonValue::as_object)
        .and_then(|obj| obj.get("enabled").and_then(JsonValue::as_bool))
        .unwrap_or(true);

    Ok(CredentialConfig {
        default_source,
        auto_discover,
        claude_code_enabled: auto_discover && claude_code_enabled,
    })
}

fn deep_merge_objects(
    target: &mut BTreeMap<String, JsonValue>,
    source: &BTreeMap<String, JsonValue>,
) {
    for (key, value) in source {
        match (target.get_mut(key), value) {
            (Some(JsonValue::Object(existing)), JsonValue::Object(incoming)) => {
                deep_merge_objects(existing, incoming);
            }
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}
