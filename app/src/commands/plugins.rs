use crate::error::{AppError, AppResult};
use aineer_plugins::{builtin_plugins, Plugin};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub kind: String,
    pub enabled: bool,
}

#[tauri::command]
pub async fn list_plugins() -> AppResult<Vec<PluginInfo>> {
    let builtins = builtin_plugins();
    Ok(builtins
        .iter()
        .map(|def| {
            let meta = def.metadata();
            PluginInfo {
                id: meta.id.clone(),
                name: meta.name.clone(),
                version: meta.version.clone(),
                description: meta.description.clone(),
                kind: format!("{:?}", meta.kind),
                enabled: meta.default_enabled,
            }
        })
        .collect())
}

#[tauri::command]
pub async fn install_plugin(name: String) -> AppResult<()> {
    tracing::info!("install_plugin: {name}");
    Err(AppError::Plugin(format!(
        "Plugin install '{name}' not yet implemented"
    )))
}

#[tauri::command]
pub async fn uninstall_plugin(name: String) -> AppResult<()> {
    tracing::info!("uninstall_plugin: {name}");
    Err(AppError::Plugin(format!(
        "Plugin uninstall '{name}' not yet implemented"
    )))
}
