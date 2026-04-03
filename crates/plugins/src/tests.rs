use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::constants::{MANIFEST_FILE_NAME, MANIFEST_RELATIVE_PATH};
use crate::*;

fn temp_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("plugins-{label}-{nanos}"))
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir");
    }
    fs::write(path, contents).expect("write file");
}

fn write_loader_plugin(root: &Path) {
    write_file(
        root.join("hooks").join("pre.sh").as_path(),
        "#!/bin/sh\nprintf 'pre'\n",
    );
    write_file(
        root.join("tools").join("echo-tool.sh").as_path(),
        "#!/bin/sh\ncat\n",
    );
    write_file(
        root.join("commands").join("sync.sh").as_path(),
        "#!/bin/sh\nprintf 'sync'\n",
    );
    write_file(
        root.join(MANIFEST_FILE_NAME).as_path(),
        r#"{
  "name": "loader-demo",
  "version": "1.2.3",
  "description": "Manifest loader test plugin",
  "permissions": ["read", "write"],
  "hooks": {
"PreToolUse": ["./hooks/pre.sh"]
  },
  "tools": [
{
  "name": "echo_tool",
  "description": "Echoes JSON input",
  "inputSchema": {
    "type": "object"
  },
  "command": "./tools/echo-tool.sh",
  "requiredPermission": "workspace-write"
}
  ],
  "commands": [
{
  "name": "sync",
  "description": "Sync command",
  "command": "./commands/sync.sh"
}
  ]
}"#,
    );
}

fn write_external_plugin(root: &Path, name: &str, version: &str) {
    write_file(
        root.join("hooks").join("pre.sh").as_path(),
        "#!/bin/sh\nprintf 'pre'\n",
    );
    write_file(
        root.join("hooks").join("post.sh").as_path(),
        "#!/bin/sh\nprintf 'post'\n",
    );
    write_file(
        root.join(MANIFEST_RELATIVE_PATH).as_path(),
        format!(
            "{{\n  \"name\": \"{name}\",\n  \"version\": \"{version}\",\n  \"description\": \"test plugin\",\n  \"hooks\": {{\n    \"PreToolUse\": [\"./hooks/pre.sh\"],\n    \"PostToolUse\": [\"./hooks/post.sh\"]\n  }}\n}}"
        )
        .as_str(),
    );
}

fn write_broken_plugin(root: &Path, name: &str) {
    write_file(
        root.join(MANIFEST_RELATIVE_PATH).as_path(),
        format!(
            "{{\n  \"name\": \"{name}\",\n  \"version\": \"1.0.0\",\n  \"description\": \"broken plugin\",\n  \"hooks\": {{\n    \"PreToolUse\": [\"./hooks/missing.sh\"]\n  }}\n}}"
        )
        .as_str(),
    );
}

fn write_lifecycle_plugin(root: &Path, name: &str, version: &str) -> PathBuf {
    let log_path = root.join("lifecycle.log");
    write_file(
        root.join("lifecycle").join("init.sh").as_path(),
        "#!/bin/sh\nprintf 'init\\n' >> lifecycle.log\n",
    );
    write_file(
        root.join("lifecycle").join("shutdown.sh").as_path(),
        "#!/bin/sh\nprintf 'shutdown\\n' >> lifecycle.log\n",
    );
    write_file(
        root.join(MANIFEST_RELATIVE_PATH).as_path(),
        format!(
            "{{\n  \"name\": \"{name}\",\n  \"version\": \"{version}\",\n  \"description\": \"lifecycle plugin\",\n  \"lifecycle\": {{\n    \"Init\": [\"./lifecycle/init.sh\"],\n    \"Shutdown\": [\"./lifecycle/shutdown.sh\"]\n  }}\n}}"
        )
        .as_str(),
    );
    log_path
}

fn write_tool_plugin(root: &Path, name: &str, version: &str) {
    write_tool_plugin_with_name(root, name, version, "plugin_echo");
}

fn write_tool_plugin_with_name(root: &Path, name: &str, version: &str, tool_name: &str) {
    let script_path = root.join("tools").join("echo-json.sh");
    write_file(
        &script_path,
        "#!/bin/sh\nINPUT=$(cat)\nprintf '{\"plugin\":\"%s\",\"tool\":\"%s\",\"input\":%s}\\n' \"$CODINEER_PLUGIN_ID\" \"$CODINEER_TOOL_NAME\" \"$INPUT\"\n",
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&script_path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script_path, permissions).expect("chmod");
    }
    write_file(
        root.join(MANIFEST_RELATIVE_PATH).as_path(),
        format!(
            "{{\n  \"name\": \"{name}\",\n  \"version\": \"{version}\",\n  \"description\": \"tool plugin\",\n  \"tools\": [\n    {{\n      \"name\": \"{tool_name}\",\n      \"description\": \"Echo JSON input\",\n      \"inputSchema\": {{\"type\": \"object\", \"properties\": {{\"message\": {{\"type\": \"string\"}}}}, \"required\": [\"message\"], \"additionalProperties\": false}},\n      \"command\": \"./tools/echo-json.sh\",\n      \"requiredPermission\": \"workspace-write\"\n    }}\n  ]\n}}"
        )
        .as_str(),
    );
}

fn write_bundled_plugin(root: &Path, name: &str, version: &str, default_enabled: bool) {
    write_file(
        root.join(MANIFEST_RELATIVE_PATH).as_path(),
        format!(
            "{{\n  \"name\": \"{name}\",\n  \"version\": \"{version}\",\n  \"description\": \"bundled plugin\",\n  \"defaultEnabled\": {}\n}}",
            if default_enabled { "true" } else { "false" }
        )
        .as_str(),
    );
}

fn load_enabled_plugins(path: &Path) -> BTreeMap<String, bool> {
    let contents = fs::read_to_string(path).expect("settings should exist");
    let root: Value = serde_json::from_str(&contents).expect("settings json");
    root.get("enabledPlugins")
        .and_then(Value::as_object)
        .map(|enabled_plugins| {
            enabled_plugins
                .iter()
                .map(|(plugin_id, value)| {
                    (
                        plugin_id.clone(),
                        value.as_bool().expect("plugin state should be a bool"),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn load_plugin_from_directory_validates_required_fields() {
    let root = temp_dir("manifest-required");
    write_file(
        root.join(MANIFEST_FILE_NAME).as_path(),
        r#"{"name":"","version":"1.0.0","description":"desc"}"#,
    );

    let error = load_plugin_from_directory(&root).expect_err("empty name should fail");
    assert!(error.to_string().contains("name cannot be empty"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_plugin_from_directory_reads_root_manifest_and_validates_entries() {
    let root = temp_dir("manifest-root");
    write_loader_plugin(&root);

    let manifest = load_plugin_from_directory(&root).expect("manifest should load");
    assert_eq!(manifest.name, "loader-demo");
    assert_eq!(manifest.version, "1.2.3");
    assert_eq!(
        manifest
            .permissions
            .iter()
            .map(|permission| permission.as_str())
            .collect::<Vec<_>>(),
        vec!["read", "write"]
    );
    assert_eq!(manifest.hooks.pre_tool_use, vec!["./hooks/pre.sh"]);
    assert_eq!(manifest.tools.len(), 1);
    assert_eq!(manifest.tools[0].name, "echo_tool");
    assert_eq!(
        manifest.tools[0].required_permission,
        PluginToolPermission::WorkspaceWrite
    );
    assert_eq!(manifest.commands.len(), 1);
    assert_eq!(manifest.commands[0].name, "sync");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_plugin_from_directory_supports_packaged_manifest_path() {
    let root = temp_dir("manifest-packaged");
    write_external_plugin(&root, "packaged-demo", "1.0.0");

    let manifest = load_plugin_from_directory(&root).expect("packaged manifest should load");
    assert_eq!(manifest.name, "packaged-demo");
    assert!(manifest.tools.is_empty());
    assert!(manifest.commands.is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_plugin_from_directory_defaults_optional_fields() {
    let root = temp_dir("manifest-defaults");
    write_file(
        root.join(MANIFEST_FILE_NAME).as_path(),
        r#"{
  "name": "minimal",
  "version": "0.1.0",
  "description": "Minimal manifest"
}"#,
    );

    let manifest = load_plugin_from_directory(&root).expect("minimal manifest should load");
    assert!(manifest.permissions.is_empty());
    assert!(manifest.hooks.is_empty());
    assert!(manifest.tools.is_empty());
    assert!(manifest.commands.is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_plugin_from_directory_rejects_duplicate_permissions_and_commands() {
    let root = temp_dir("manifest-duplicates");
    write_file(
        root.join("commands").join("sync.sh").as_path(),
        "#!/bin/sh\nprintf 'sync'\n",
    );
    write_file(
        root.join(MANIFEST_FILE_NAME).as_path(),
        r#"{
  "name": "duplicate-manifest",
  "version": "1.0.0",
  "description": "Duplicate validation",
  "permissions": ["read", "read"],
  "commands": [
{"name": "sync", "description": "Sync one", "command": "./commands/sync.sh"},
{"name": "sync", "description": "Sync two", "command": "./commands/sync.sh"}
  ]
}"#,
    );

    let error = load_plugin_from_directory(&root).expect_err("duplicates should fail");
    match error {
        PluginError::ManifestValidation(errors) => {
            assert!(errors.iter().any(|error| matches!(
                error,
                PluginManifestValidationError::DuplicatePermission { permission }
                if permission == "read"
            )));
            assert!(errors.iter().any(|error| matches!(
                error,
                PluginManifestValidationError::DuplicateEntry { kind, name }
                if *kind == "command" && name == "sync"
            )));
        }
        other => panic!("expected manifest validation errors, got {other}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_plugin_from_directory_rejects_missing_tool_or_command_paths() {
    let root = temp_dir("manifest-paths");
    write_file(
        root.join(MANIFEST_FILE_NAME).as_path(),
        r#"{
  "name": "missing-paths",
  "version": "1.0.0",
  "description": "Missing path validation",
  "tools": [
{
  "name": "tool_one",
  "description": "Missing tool script",
  "inputSchema": {"type": "object"},
  "command": "./tools/missing.sh"
}
  ]
}"#,
    );

    let error = load_plugin_from_directory(&root).expect_err("missing paths should fail");
    assert!(error.to_string().contains("does not exist"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_plugin_from_directory_rejects_invalid_permissions() {
    let root = temp_dir("manifest-invalid-permissions");
    write_file(
        root.join(MANIFEST_FILE_NAME).as_path(),
        r#"{
  "name": "invalid-permissions",
  "version": "1.0.0",
  "description": "Invalid permission validation",
  "permissions": ["admin"]
}"#,
    );

    let error = load_plugin_from_directory(&root).expect_err("invalid permissions should fail");
    match error {
        PluginError::ManifestValidation(errors) => {
            assert!(errors.iter().any(|error| matches!(
                error,
                PluginManifestValidationError::InvalidPermission { permission }
                if permission == "admin"
            )));
        }
        other => panic!("expected manifest validation errors, got {other}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_plugin_from_directory_rejects_invalid_tool_required_permission() {
    let root = temp_dir("manifest-invalid-tool-permission");
    write_file(
        root.join("tools").join("echo.sh").as_path(),
        "#!/bin/sh\ncat\n",
    );
    write_file(
        root.join(MANIFEST_FILE_NAME).as_path(),
        r#"{
  "name": "invalid-tool-permission",
  "version": "1.0.0",
  "description": "Invalid tool permission validation",
  "tools": [
{
  "name": "echo_tool",
  "description": "Echo tool",
  "inputSchema": {"type": "object"},
  "command": "./tools/echo.sh",
  "requiredPermission": "admin"
}
  ]
}"#,
    );

    let error = load_plugin_from_directory(&root).expect_err("invalid tool permission should fail");
    match error {
        PluginError::ManifestValidation(errors) => {
            assert!(errors.iter().any(|error| matches!(
                error,
                PluginManifestValidationError::InvalidToolRequiredPermission {
                    tool_name,
                    permission
                } if tool_name == "echo_tool" && permission == "admin"
            )));
        }
        other => panic!("expected manifest validation errors, got {other}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_plugin_from_directory_accumulates_multiple_validation_errors() {
    let root = temp_dir("manifest-multi-error");
    write_file(
        root.join(MANIFEST_FILE_NAME).as_path(),
        r#"{
  "name": "",
  "version": "1.0.0",
  "description": "",
  "permissions": ["admin"],
  "commands": [
{"name": "", "description": "", "command": "./commands/missing.sh"}
  ]
}"#,
    );

    let error =
        load_plugin_from_directory(&root).expect_err("multiple manifest errors should fail");
    match error {
        PluginError::ManifestValidation(errors) => {
            assert!(errors.len() >= 4);
            assert!(errors.iter().any(|error| matches!(
                error,
                PluginManifestValidationError::EmptyField { field } if *field == "name"
            )));
            assert!(errors.iter().any(|error| matches!(
                error,
                PluginManifestValidationError::EmptyField { field }
                if *field == "description"
            )));
            assert!(errors.iter().any(|error| matches!(
                error,
                PluginManifestValidationError::InvalidPermission { permission }
                if permission == "admin"
            )));
        }
        other => panic!("expected manifest validation errors, got {other}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn discovers_builtin_and_bundled_plugins() {
    let manager = PluginManager::new(PluginManagerConfig::new(temp_dir("discover")));
    let plugins = manager.list_plugins().expect("plugins should list");
    assert!(plugins
        .iter()
        .any(|plugin| plugin.metadata.kind == PluginKind::Builtin));
    assert!(plugins
        .iter()
        .any(|plugin| plugin.metadata.kind == PluginKind::Bundled));
}

#[test]
fn installs_enables_updates_and_uninstalls_external_plugins() {
    let config_home = temp_dir("home");
    let source_root = temp_dir("source");
    write_external_plugin(&source_root, "demo", "1.0.0");

    let mut manager = PluginManager::new(PluginManagerConfig::new(&config_home));
    let install = manager
        .install(source_root.to_str().expect("utf8 path"))
        .expect("install should succeed");
    assert_eq!(install.plugin_id, "demo@external");
    assert!(manager
        .list_plugins()
        .expect("list plugins")
        .iter()
        .any(|plugin| plugin.metadata.id == "demo@external" && plugin.enabled));

    let hooks = manager.aggregated_hooks().expect("hooks should aggregate");
    assert_eq!(hooks.pre_tool_use.len(), 1);
    assert!(hooks.pre_tool_use[0].contains("pre.sh"));

    manager
        .disable("demo@external")
        .expect("disable should work");
    assert!(manager
        .aggregated_hooks()
        .expect("hooks after disable")
        .is_empty());
    manager.enable("demo@external").expect("enable should work");

    write_external_plugin(&source_root, "demo", "2.0.0");
    let update = manager.update("demo@external").expect("update should work");
    assert_eq!(update.old_version, "1.0.0");
    assert_eq!(update.new_version, "2.0.0");

    manager
        .uninstall("demo@external")
        .expect("uninstall should work");
    assert!(!manager
        .list_plugins()
        .expect("list plugins")
        .iter()
        .any(|plugin| plugin.metadata.id == "demo@external"));

    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(source_root);
}

#[test]
fn auto_installs_bundled_plugins_into_the_registry() {
    let config_home = temp_dir("bundled-home");
    let bundled_root = temp_dir("bundled-root");
    write_bundled_plugin(&bundled_root.join("starter"), "starter", "0.1.0", false);

    let mut config = PluginManagerConfig::new(&config_home);
    config.bundled_root = Some(bundled_root.clone());
    let manager = PluginManager::new(config);

    let installed = manager
        .list_installed_plugins()
        .expect("bundled plugins should auto-install");
    assert!(installed.iter().any(|plugin| {
        plugin.metadata.id == "starter@bundled"
            && plugin.metadata.kind == PluginKind::Bundled
            && !plugin.enabled
    }));

    let registry = manager.load_registry().expect("registry should exist");
    let record = registry
        .plugins
        .get("starter@bundled")
        .expect("bundled plugin should be recorded");
    assert_eq!(record.kind, PluginKind::Bundled);
    assert!(record.install_path.exists());

    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(bundled_root);
}

#[test]
fn default_bundled_root_loads_repo_bundles_as_installed_plugins() {
    let config_home = temp_dir("default-bundled-home");
    let manager = PluginManager::new(PluginManagerConfig::new(&config_home));

    let installed = manager
        .list_installed_plugins()
        .expect("default bundled plugins should auto-install");
    assert!(installed
        .iter()
        .any(|plugin| plugin.metadata.id == "example-bundled@bundled"));
    assert!(installed
        .iter()
        .any(|plugin| plugin.metadata.id == "sample-hooks@bundled"));

    let _ = fs::remove_dir_all(config_home);
}

#[test]
fn bundled_sync_prunes_removed_bundled_registry_entries() {
    let config_home = temp_dir("bundled-prune-home");
    let bundled_root = temp_dir("bundled-prune-root");
    let stale_install_path = config_home
        .join("plugins")
        .join("installed")
        .join("stale-bundled-external");
    write_bundled_plugin(&bundled_root.join("active"), "active", "0.1.0", false);
    write_file(
        stale_install_path.join(MANIFEST_RELATIVE_PATH).as_path(),
        r#"{
  "name": "stale",
  "version": "0.1.0",
  "description": "stale bundled plugin"
}"#,
    );

    let mut config = PluginManagerConfig::new(&config_home);
    config.bundled_root = Some(bundled_root.clone());
    config.install_root = Some(config_home.join("plugins").join("installed"));
    let manager = PluginManager::new(config);

    let mut registry = InstalledPluginRegistry::default();
    registry.plugins.insert(
        "stale@bundled".to_string(),
        InstalledPluginRecord {
            kind: PluginKind::Bundled,
            id: "stale@bundled".to_string(),
            name: "stale".to_string(),
            version: "0.1.0".to_string(),
            description: "stale bundled plugin".to_string(),
            install_path: stale_install_path.clone(),
            source: PluginInstallSource::LocalPath {
                path: bundled_root.join("stale"),
            },
            installed_at_unix_ms: 1,
            updated_at_unix_ms: 1,
        },
    );
    manager.store_registry(&registry).expect("store registry");
    manager
        .write_enabled_state("stale@bundled", Some(true))
        .expect("seed bundled enabled state");

    let installed = manager
        .list_installed_plugins()
        .expect("bundled sync should succeed");
    assert!(installed
        .iter()
        .any(|plugin| plugin.metadata.id == "active@bundled"));
    assert!(!installed
        .iter()
        .any(|plugin| plugin.metadata.id == "stale@bundled"));

    let registry = manager.load_registry().expect("load registry");
    assert!(!registry.plugins.contains_key("stale@bundled"));
    assert!(!stale_install_path.exists());

    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(bundled_root);
}

#[test]
fn installed_plugin_discovery_keeps_registry_entries_outside_install_root() {
    let config_home = temp_dir("registry-fallback-home");
    let bundled_root = temp_dir("registry-fallback-bundled");
    let install_root = config_home.join("plugins").join("installed");
    let external_install_path = temp_dir("registry-fallback-external");
    write_file(
        external_install_path.join(MANIFEST_FILE_NAME).as_path(),
        r#"{
  "name": "registry-fallback",
  "version": "1.0.0",
  "description": "Registry fallback plugin"
}"#,
    );

    let mut config = PluginManagerConfig::new(&config_home);
    config.bundled_root = Some(bundled_root.clone());
    config.install_root = Some(install_root.clone());
    let manager = PluginManager::new(config);

    let mut registry = InstalledPluginRegistry::default();
    registry.plugins.insert(
        "registry-fallback@external".to_string(),
        InstalledPluginRecord {
            kind: PluginKind::External,
            id: "registry-fallback@external".to_string(),
            name: "registry-fallback".to_string(),
            version: "1.0.0".to_string(),
            description: "Registry fallback plugin".to_string(),
            install_path: external_install_path.clone(),
            source: PluginInstallSource::LocalPath {
                path: external_install_path.clone(),
            },
            installed_at_unix_ms: 1,
            updated_at_unix_ms: 1,
        },
    );
    manager.store_registry(&registry).expect("store registry");
    manager
        .write_enabled_state("stale-external@external", Some(true))
        .expect("seed stale external enabled state");

    let installed = manager
        .list_installed_plugins()
        .expect("registry fallback plugin should load");
    assert!(installed
        .iter()
        .any(|plugin| plugin.metadata.id == "registry-fallback@external"));

    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(bundled_root);
    let _ = fs::remove_dir_all(external_install_path);
}

#[test]
fn installed_plugin_discovery_prunes_stale_registry_entries() {
    let config_home = temp_dir("registry-prune-home");
    let bundled_root = temp_dir("registry-prune-bundled");
    let install_root = config_home.join("plugins").join("installed");
    let missing_install_path = temp_dir("registry-prune-missing");

    let mut config = PluginManagerConfig::new(&config_home);
    config.bundled_root = Some(bundled_root.clone());
    config.install_root = Some(install_root);
    let manager = PluginManager::new(config);

    let mut registry = InstalledPluginRegistry::default();
    registry.plugins.insert(
        "stale-external@external".to_string(),
        InstalledPluginRecord {
            kind: PluginKind::External,
            id: "stale-external@external".to_string(),
            name: "stale-external".to_string(),
            version: "1.0.0".to_string(),
            description: "stale external plugin".to_string(),
            install_path: missing_install_path.clone(),
            source: PluginInstallSource::LocalPath {
                path: missing_install_path.clone(),
            },
            installed_at_unix_ms: 1,
            updated_at_unix_ms: 1,
        },
    );
    manager.store_registry(&registry).expect("store registry");

    let installed = manager
        .list_installed_plugins()
        .expect("stale registry entries should be pruned");
    assert!(!installed
        .iter()
        .any(|plugin| plugin.metadata.id == "stale-external@external"));

    let registry = manager.load_registry().expect("load registry");
    assert!(!registry.plugins.contains_key("stale-external@external"));

    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(bundled_root);
}

#[test]
fn persists_bundled_plugin_enable_state_across_reloads() {
    let config_home = temp_dir("bundled-state-home");
    let bundled_root = temp_dir("bundled-state-root");
    write_bundled_plugin(&bundled_root.join("starter"), "starter", "0.1.0", false);

    let mut config = PluginManagerConfig::new(&config_home);
    config.bundled_root = Some(bundled_root.clone());
    let mut manager = PluginManager::new(config.clone());

    manager
        .enable("starter@bundled")
        .expect("enable bundled plugin should succeed");
    assert_eq!(
        load_enabled_plugins(&manager.settings_path()).get("starter@bundled"),
        Some(&true)
    );

    let mut reloaded_config = PluginManagerConfig::new(&config_home);
    reloaded_config.bundled_root = Some(bundled_root.clone());
    reloaded_config.enabled_plugins = load_enabled_plugins(&manager.settings_path());
    let reloaded_manager = PluginManager::new(reloaded_config);
    let reloaded = reloaded_manager
        .list_installed_plugins()
        .expect("bundled plugins should still be listed");
    assert!(reloaded
        .iter()
        .any(|plugin| { plugin.metadata.id == "starter@bundled" && plugin.enabled }));

    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(bundled_root);
}

#[test]
fn persists_bundled_plugin_disable_state_across_reloads() {
    let config_home = temp_dir("bundled-disabled-home");
    let bundled_root = temp_dir("bundled-disabled-root");
    write_bundled_plugin(&bundled_root.join("starter"), "starter", "0.1.0", true);

    let mut config = PluginManagerConfig::new(&config_home);
    config.bundled_root = Some(bundled_root.clone());
    let mut manager = PluginManager::new(config);

    manager
        .disable("starter@bundled")
        .expect("disable bundled plugin should succeed");
    assert_eq!(
        load_enabled_plugins(&manager.settings_path()).get("starter@bundled"),
        Some(&false)
    );

    let mut reloaded_config = PluginManagerConfig::new(&config_home);
    reloaded_config.bundled_root = Some(bundled_root.clone());
    reloaded_config.enabled_plugins = load_enabled_plugins(&manager.settings_path());
    let reloaded_manager = PluginManager::new(reloaded_config);
    let reloaded = reloaded_manager
        .list_installed_plugins()
        .expect("bundled plugins should still be listed");
    assert!(reloaded
        .iter()
        .any(|plugin| { plugin.metadata.id == "starter@bundled" && !plugin.enabled }));

    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(bundled_root);
}

#[test]
fn validates_plugin_source_before_install() {
    let config_home = temp_dir("validate-home");
    let source_root = temp_dir("validate-source");
    write_external_plugin(&source_root, "validator", "1.0.0");
    let manager = PluginManager::new(PluginManagerConfig::new(&config_home));
    let manifest = manager
        .validate_plugin_source(source_root.to_str().expect("utf8 path"))
        .expect("manifest should validate");
    assert_eq!(manifest.name, "validator");
    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(source_root);
}

#[test]
fn plugin_registry_tracks_enabled_state_and_lookup() {
    let config_home = temp_dir("registry-home");
    let source_root = temp_dir("registry-source");
    write_external_plugin(&source_root, "registry-demo", "1.0.0");

    let mut manager = PluginManager::new(PluginManagerConfig::new(&config_home));
    manager
        .install(source_root.to_str().expect("utf8 path"))
        .expect("install should succeed");
    manager
        .disable("registry-demo@external")
        .expect("disable should succeed");

    let registry = manager.plugin_registry().expect("registry should build");
    let plugin = registry
        .get("registry-demo@external")
        .expect("installed plugin should be discoverable");
    assert_eq!(plugin.metadata().name, "registry-demo");
    assert!(!plugin.is_enabled());
    assert!(registry.contains("registry-demo@external"));
    assert!(!registry.contains("missing@external"));

    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(source_root);
}

#[test]
fn rejects_plugin_sources_with_missing_hook_paths() {
    let config_home = temp_dir("broken-home");
    let source_root = temp_dir("broken-source");
    write_broken_plugin(&source_root, "broken");

    let manager = PluginManager::new(PluginManagerConfig::new(&config_home));
    let error = manager
        .validate_plugin_source(source_root.to_str().expect("utf8 path"))
        .expect_err("missing hook file should fail validation");
    assert!(error.to_string().contains("does not exist"));

    let mut manager = PluginManager::new(PluginManagerConfig::new(&config_home));
    let install_error = manager
        .install(source_root.to_str().expect("utf8 path"))
        .expect_err("install should reject invalid hook paths");
    assert!(install_error.to_string().contains("does not exist"));

    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(source_root);
}

#[test]
#[cfg(unix)]
fn plugin_registry_runs_initialize_and_shutdown_for_enabled_plugins() {
    let config_home = temp_dir("lifecycle-home");
    let source_root = temp_dir("lifecycle-source");
    let _ = write_lifecycle_plugin(&source_root, "lifecycle-demo", "1.0.0");

    let mut manager = PluginManager::new(PluginManagerConfig::new(&config_home));
    let install = manager
        .install(source_root.to_str().expect("utf8 path"))
        .expect("install should succeed");
    let log_path = install.install_path.join("lifecycle.log");

    let registry = manager.plugin_registry().expect("registry should build");
    registry.initialize().expect("init should succeed");
    registry.shutdown().expect("shutdown should succeed");

    let log = fs::read_to_string(&log_path).expect("lifecycle log should exist");
    assert_eq!(log, "init\nshutdown\n");

    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(source_root);
}

#[test]
#[cfg(unix)]
fn aggregates_and_executes_plugin_tools() {
    let config_home = temp_dir("tool-home");
    let source_root = temp_dir("tool-source");
    write_tool_plugin(&source_root, "tool-demo", "1.0.0");

    let mut manager = PluginManager::new(PluginManagerConfig::new(&config_home));
    manager
        .install(source_root.to_str().expect("utf8 path"))
        .expect("install should succeed");

    let tools = manager.aggregated_tools().expect("tools should aggregate");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].definition().name, "plugin_echo");
    assert_eq!(tools[0].required_permission(), "workspace-write");

    let output = tools[0]
        .execute(&serde_json::json!({ "message": "hello" }))
        .expect("plugin tool should execute");
    let payload: Value = serde_json::from_str(&output).expect("valid json");
    assert_eq!(payload["plugin"], "tool-demo@external");
    assert_eq!(payload["tool"], "plugin_echo");
    assert_eq!(payload["input"]["message"], "hello");

    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(source_root);
}

#[test]
fn list_installed_plugins_scans_install_root_without_registry_entries() {
    let config_home = temp_dir("installed-scan-home");
    let bundled_root = temp_dir("installed-scan-bundled");
    let install_root = config_home.join("plugins").join("installed");
    let installed_plugin_root = install_root.join("scan-demo");
    write_file(
        installed_plugin_root.join(MANIFEST_FILE_NAME).as_path(),
        r#"{
  "name": "scan-demo",
  "version": "1.0.0",
  "description": "Scanned from install root"
}"#,
    );

    let mut config = PluginManagerConfig::new(&config_home);
    config.bundled_root = Some(bundled_root.clone());
    config.install_root = Some(install_root);
    let manager = PluginManager::new(config);

    let installed = manager
        .list_installed_plugins()
        .expect("installed plugins should scan directories");
    assert!(installed
        .iter()
        .any(|plugin| plugin.metadata.id == "scan-demo@external"));

    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(bundled_root);
}

#[test]
fn list_installed_plugins_scans_packaged_manifests_in_install_root() {
    let config_home = temp_dir("installed-packaged-scan-home");
    let bundled_root = temp_dir("installed-packaged-scan-bundled");
    let install_root = config_home.join("plugins").join("installed");
    let installed_plugin_root = install_root.join("scan-packaged");
    write_file(
        installed_plugin_root.join(MANIFEST_RELATIVE_PATH).as_path(),
        r#"{
  "name": "scan-packaged",
  "version": "1.0.0",
  "description": "Packaged manifest in install root"
}"#,
    );

    let mut config = PluginManagerConfig::new(&config_home);
    config.bundled_root = Some(bundled_root.clone());
    config.install_root = Some(install_root);
    let manager = PluginManager::new(config);

    let installed = manager
        .list_installed_plugins()
        .expect("installed plugins should scan packaged manifests");
    assert!(installed
        .iter()
        .any(|plugin| plugin.metadata.id == "scan-packaged@external"));

    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(bundled_root);
}
