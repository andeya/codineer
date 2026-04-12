use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

#[tauri::command]
pub async fn get_project_root() -> AppResult<String> {
    let cwd = std::env::current_dir().map_err(|e| AppError::File(e.to_string()))?;
    // Walk up to find the workspace root (directory containing workspace Cargo.toml)
    let mut dir = cwd.as_path();
    loop {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") {
                    return Ok(dir.to_string_lossy().into_owned());
                }
            }
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }
    // If the process CWD looks like a non-user directory (e.g. "/" on macOS
    // when launched from Finder), fall back to $HOME.
    let cwd_str = cwd.to_string_lossy();
    if cwd_str == "/" || cwd_str.starts_with("/Applications") || cwd_str.starts_with("/System") {
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(&home);
            if home_path.is_dir() {
                return Ok(home);
            }
        }
    }
    Ok(cwd_str.into_owned())
}

#[tauri::command]
pub async fn list_dir(path: String) -> AppResult<Vec<FileEntry>> {
    let dir = PathBuf::from(&path);
    if !dir.is_dir() {
        return Err(AppError::File(format!("Not a directory: {}", path)));
    }
    let mut entries = Vec::new();
    let read_dir = std::fs::read_dir(&dir).map_err(|e| AppError::File(e.to_string()))?;
    for entry in read_dir.flatten() {
        let meta = entry.metadata().ok();
        entries.push(FileEntry {
            name: entry.file_name().to_string_lossy().into(),
            path: entry.path().to_string_lossy().into(),
            is_dir: meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
            size: meta
                .as_ref()
                .and_then(|m| if m.is_file() { Some(m.len()) } else { None }),
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

#[tauri::command]
pub async fn read_file(path: String) -> AppResult<String> {
    std::fs::read_to_string(&path)
        .map_err(|e| AppError::File(format!("Failed to read {}: {}", path, e)))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub is_dir: bool,
    pub matches: Vec<ContentMatch>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContentMatch {
    pub line_number: usize,
    pub line: String,
}

const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target", "dist", ".DS_Store", "gen"];

const MAX_RESULTS: usize = 200;

#[tauri::command]
pub async fn search_files(
    dir: String,
    query: String,
    search_content: bool,
) -> AppResult<Vec<SearchResult>> {
    tokio::task::spawn_blocking(move || {
        let query_lower = query.to_lowercase();
        let root = PathBuf::from(&dir);
        let mut results = Vec::new();
        collect_search_results(&root, &root, &query_lower, search_content, &mut results);
        results.truncate(MAX_RESULTS);
        results
    })
    .await
    .map_err(|e| AppError::File(format!("Search task failed: {e}")))
}

fn collect_search_results(
    root: &Path,
    dir: &Path,
    query: &str,
    search_content: bool,
    results: &mut Vec<SearchResult>,
) {
    if results.len() >= MAX_RESULTS {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if results.len() >= MAX_RESULTS {
            return;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let is_dir = path.is_dir();

        // Path matching
        if rel_path.to_lowercase().contains(query) || name.to_lowercase().contains(query) {
            if search_content && !is_dir {
                let matches = search_file_content(&path, query);
                results.push(SearchResult {
                    path: rel_path,
                    is_dir,
                    matches,
                });
            } else {
                results.push(SearchResult {
                    path: rel_path,
                    is_dir,
                    matches: vec![],
                });
            }
        } else if search_content && !is_dir {
            let matches = search_file_content(&path, query);
            if !matches.is_empty() {
                results.push(SearchResult {
                    path: rel_path,
                    is_dir,
                    matches,
                });
            }
        }

        if is_dir {
            collect_search_results(root, &path, query, search_content, results);
        }
    }
}

fn search_file_content(path: &Path, query: &str) -> Vec<ContentMatch> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return vec![];
    };
    // Skip binary/huge files
    if content.len() > 1_000_000 {
        return vec![];
    }
    let mut matches = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if line.to_lowercase().contains(query) {
            matches.push(ContentMatch {
                line_number: i + 1,
                line: truncate_line(line, 200),
            });
            if matches.len() >= 5 {
                break;
            }
        }
    }
    matches
}

fn truncate_line(line: &str, max_chars: usize) -> String {
    let char_count = line.chars().count();
    if char_count <= max_chars {
        return line.to_string();
    }
    let truncated: String = line.chars().take(max_chars).collect();
    format!("{truncated}...")
}
