use std::env;
use std::path::{Path, PathBuf};

use plugins::{PluginManager, PluginManagerConfig};
use runtime::ConfigLoader;
use tools::GlobalToolRegistry;

pub(crate) fn build_runtime_plugin_state(
) -> Result<(runtime::RuntimeConfig, GlobalToolRegistry), Box<dyn std::error::Error>> {
    crate::init::ensure_home_codineer_dirs();
    let cwd = env::current_dir()?;
    let loader = ConfigLoader::default_for(&cwd);
    let runtime_config = loader.load()?;
    runtime_config.apply_env();
    let plugin_manager = build_plugin_manager(&cwd, &loader, &runtime_config);
    let tool_registry = GlobalToolRegistry::with_plugin_tools(plugin_manager.aggregated_tools()?)?;
    Ok((runtime_config, tool_registry))
}

pub(crate) fn build_plugin_manager(
    cwd: &Path,
    loader: &ConfigLoader,
    runtime_config: &runtime::RuntimeConfig,
) -> PluginManager {
    let plugin_settings = runtime_config.plugins();
    let mut plugin_config = PluginManagerConfig::new(loader.config_home().to_path_buf());
    plugin_config.enabled_plugins = plugin_settings.enabled_plugins().clone();
    plugin_config.external_dirs = plugin_settings
        .external_directories()
        .iter()
        .map(|path| resolve_plugin_path(cwd, loader.config_home(), path))
        .collect();
    plugin_config.install_root = plugin_settings
        .install_root()
        .map(|path| resolve_plugin_path(cwd, loader.config_home(), path));
    plugin_config.registry_path = plugin_settings
        .registry_path()
        .map(|path| resolve_plugin_path(cwd, loader.config_home(), path));
    plugin_config.bundled_root = plugin_settings
        .bundled_root()
        .map(|path| resolve_plugin_path(cwd, loader.config_home(), path));
    PluginManager::new(plugin_config)
}

fn resolve_plugin_path(cwd: &Path, config_home: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else if value.starts_with('.') {
        cwd.join(path)
    } else {
        config_home.join(path)
    }
}

pub(crate) fn build_system_prompt() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    build_system_prompt_with_lsp(None)
}

pub(crate) fn build_system_prompt_with_lsp(
    lsp_context: Option<&runtime::LspContextEnrichment>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    Ok(runtime::load_system_prompt_with_lsp(
        env::current_dir()?,
        super::current_date(),
        env::consts::OS,
        "unknown",
        lsp_context,
    )?)
}
