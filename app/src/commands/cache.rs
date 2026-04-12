use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

fn attachments_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("Cannot resolve cache dir: {e}"))?
        .join("attachments");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn history_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve data dir: {e}"))?
        .join("history");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn dir_stats(dir: &PathBuf) -> (usize, u64) {
    let mut count = 0usize;
    let mut size = 0u64;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    count += 1;
                    size += meta.len();
                }
            }
        }
    }
    (count, size)
}

fn clear_dir_contents(dir: &PathBuf) -> Result<(), String> {
    if dir.exists() {
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
            if entry.path().is_file() {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStats {
    pub attachments_count: usize,
    pub attachments_size_bytes: u64,
    pub history_count: usize,
    pub history_size_bytes: u64,
    pub cache_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatHistoryEntry {
    pub session_id: String,
    pub size_bytes: u64,
    pub modified_at: u64,
}

#[tauri::command]
pub async fn get_cache_stats(app: tauri::AppHandle) -> AppResult<CacheStats> {
    let att_dir = attachments_dir(&app).map_err(AppError::Cache)?;
    let hist_dir = history_dir(&app).map_err(AppError::Cache)?;
    let (att_count, att_size) = dir_stats(&att_dir);
    let (hist_count, hist_size) = dir_stats(&hist_dir);

    Ok(CacheStats {
        attachments_count: att_count,
        attachments_size_bytes: att_size,
        history_count: hist_count,
        history_size_bytes: hist_size,
        cache_path: att_dir
            .parent()
            .unwrap_or(&att_dir)
            .to_string_lossy()
            .to_string(),
    })
}

#[tauri::command]
pub async fn save_attachment(
    app: tauri::AppHandle,
    name: String,
    data_base64: String,
) -> AppResult<String> {
    use std::time::SystemTime;

    let bytes = base64_decode(&data_base64).map_err(AppError::Cache)?;
    let dir = attachments_dir(&app).map_err(AppError::Cache)?;
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let safe_name = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect::<String>();
    let filename = format!("{ts}_{safe_name}");
    let path = dir.join(&filename);
    fs::write(&path, bytes).map_err(|e| AppError::Cache(e.to_string()))?;
    Ok(path.to_string_lossy().to_string())
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    const TABLE: [u8; 128] = {
        let mut t = [255u8; 128];
        let mut i = 0u8;
        while i < 26 {
            t[(b'A' + i) as usize] = i;
            t[(b'a' + i) as usize] = i + 26;
            i += 1;
        }
        let mut d = 0u8;
        while d < 10 {
            t[(b'0' + d) as usize] = d + 52;
            d += 1;
        }
        t[b'+' as usize] = 62;
        t[b'/' as usize] = 63;
        t
    };

    let src = input.as_bytes();
    let mut out = Vec::with_capacity(src.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &b in src {
        if b == b'=' || b == b'\n' || b == b'\r' || b == b' ' {
            continue;
        }
        if b >= 128 || TABLE[b as usize] == 255 {
            return Err(format!("Invalid base64 character: {b}"));
        }
        buf = (buf << 6) | TABLE[b as usize] as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn clear_cache(app: tauri::AppHandle, target: String) -> AppResult<()> {
    if target == "attachments" || target == "all" {
        clear_dir_contents(
            &attachments_dir(&app).map_err(AppError::Cache)?,
        )
        .map_err(AppError::Cache)?;
    }
    if target == "history" || target == "all" {
        clear_dir_contents(&history_dir(&app).map_err(AppError::Cache)?)
            .map_err(AppError::Cache)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_chat_history(app: tauri::AppHandle) -> AppResult<Vec<ChatHistoryEntry>> {
    let dir = history_dir(&app).map_err(AppError::Cache)?;
    let mut entries = Vec::new();
    if let Ok(read_dir) = fs::read_dir(&dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Ok(meta) = entry.metadata() {
                    let modified = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    entries.push(ChatHistoryEntry {
                        session_id: path
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        size_bytes: meta.len(),
                        modified_at: modified,
                    });
                }
            }
        }
    }
    entries.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    Ok(entries)
}

#[tauri::command]
pub async fn delete_chat_history(
    app: tauri::AppHandle,
    session_id: String,
) -> AppResult<()> {
    let dir = history_dir(&app).map_err(AppError::Cache)?;
    let path = dir.join(format!("{session_id}.json"));
    if path.exists() {
        fs::remove_file(&path).map_err(|e| AppError::Cache(e.to_string()))?;
    }
    Ok(())
}
