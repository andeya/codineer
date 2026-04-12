use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub updated_at: String,
}

fn sessions_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data dir: {e}"))?;
    let dir = base.join("sessions");
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create sessions dir: {e}"))?;
    }
    Ok(dir)
}

#[tauri::command]
pub async fn save_session(
    app: tauri::AppHandle,
    data: serde_json::Value,
) -> AppResult<String> {
    let id = data
        .get("id")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| format!("session-{}", chrono::Utc::now().timestamp_millis()));

    tracing::info!("save_session: id={id}");
    let dir = sessions_dir(&app).map_err(AppError::Session)?;
    let path = dir.join(format!("{id}.json"));
    let json = serde_json::to_string_pretty(&data).map_err(|e| AppError::Session(e.to_string()))?;
    std::fs::write(&path, json)
        .map_err(|e| AppError::Session(format!("Write failed: {e}")))?;
    Ok(id)
}

#[tauri::command]
pub async fn load_session(
    app: tauri::AppHandle,
    id: String,
) -> AppResult<serde_json::Value> {
    tracing::info!("load_session: id={id}");
    let dir = sessions_dir(&app).map_err(AppError::Session)?;
    let path = dir.join(format!("{id}.json"));
    if !path.exists() {
        return Err(AppError::Session(format!("Session '{id}' not found")));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| AppError::Session(e.to_string()))?;
    serde_json::from_str(&content).map_err(|e| AppError::Session(e.to_string()))
}

#[tauri::command]
pub async fn list_sessions(app: tauri::AppHandle) -> AppResult<Vec<SessionInfo>> {
    tracing::info!("list_sessions");
    let dir = sessions_dir(&app).map_err(AppError::Session)?;
    let mut sessions = Vec::new();

    let entries = std::fs::read_dir(&dir).map_err(|e| AppError::Session(e.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|e| AppError::Session(e.to_string()))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let metadata = std::fs::metadata(&path).map_err(|e| AppError::Session(e.to_string()))?;
        let modified = metadata
            .modified()
            .map_err(|e| AppError::Session(e.to_string()))?;
        let updated_at = chrono::DateTime::<chrono::Utc>::from(modified).to_rfc3339();

        let title = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("title").and_then(|t| t.as_str()).map(String::from))
            .unwrap_or_else(|| id.clone());

        sessions.push(SessionInfo {
            id,
            title,
            updated_at,
        });
    }

    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(sessions)
}
