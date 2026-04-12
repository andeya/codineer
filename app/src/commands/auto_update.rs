use crate::error::{AppError, AppResult};
use aineer_auto_update::{AutoUpdater, UpdateInfo};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub available: bool,
    pub version: Option<String>,
    pub download_url: Option<String>,
    pub release_notes: Option<String>,
}

impl From<Option<UpdateInfo>> for UpdateCheckResult {
    fn from(info: Option<UpdateInfo>) -> Self {
        match info {
            Some(u) => Self {
                available: true,
                version: Some(u.version),
                download_url: Some(u.download_url),
                release_notes: u.release_notes,
            },
            None => Self {
                available: false,
                version: None,
                download_url: None,
                release_notes: None,
            },
        }
    }
}

#[tauri::command]
pub async fn check_for_update() -> AppResult<UpdateCheckResult> {
    let updater = AutoUpdater::new(env!("CARGO_PKG_VERSION").to_string());
    let result = updater
        .check_for_update()
        .await
        .map_err(|e| AppError::Update(e.to_string()))?;
    Ok(UpdateCheckResult::from(result))
}

#[tauri::command]
pub fn get_update_channel() -> String {
    aineer_release_channel::ReleaseChannel::current().to_string()
}
