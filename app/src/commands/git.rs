use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct GitStatus {
    pub branch: Option<String>,
    pub changed_files: Vec<GitFileStatus>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitFileStatus {
    pub path: String,
    pub status: String,
}

#[tauri::command]
pub async fn git_status(cwd: String) -> AppResult<GitStatus> {
    let branch = read_branch(&cwd).ok();

    let output = Command::new("git")
        .args(["status", "--porcelain", "-uall"])
        .current_dir(&cwd)
        .output()
        .map_err(|e| AppError::Git(format!("Failed to run git status: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Git(format!("git status failed: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let changed_files = stdout
        .lines()
        .filter(|line| line.len() >= 4)
        .map(|line| {
            let status_code = line[..2].trim().to_string();
            let file_path = line[3..].to_string();
            GitFileStatus {
                path: file_path,
                status: status_code,
            }
        })
        .collect();

    Ok(GitStatus {
        branch,
        changed_files,
    })
}

#[tauri::command]
pub async fn git_branch(cwd: String) -> AppResult<Option<String>> {
    read_branch(&cwd).map(Some).map_err(AppError::Git)
}

#[tauri::command]
pub async fn git_diff(cwd: String, path: String) -> AppResult<String> {
    let output = Command::new("git")
        .args(["diff", "--", &path])
        .current_dir(&cwd)
        .output()
        .map_err(|e| AppError::Git(format!("Failed to run git diff: {e}")))?;

    if !output.status.success() {
        // For untracked files, show the file content instead
        let unstaged = Command::new("git")
            .args(["diff", "--no-index", "/dev/null", &path])
            .current_dir(&cwd)
            .output()
            .map_err(|e| AppError::Git(format!("Failed to run git diff --no-index: {e}")))?;
        return Ok(String::from_utf8_lossy(&unstaged.stdout).into_owned());
    }

    let diff_text = String::from_utf8_lossy(&output.stdout).into_owned();

    // If staged diff is empty, try cached (staged) diff
    if diff_text.is_empty() {
        let cached = Command::new("git")
            .args(["diff", "--cached", "--", &path])
            .current_dir(&cwd)
            .output()
            .map_err(|e| AppError::Git(format!("Failed to run git diff --cached: {e}")))?;
        let cached_text = String::from_utf8_lossy(&cached.stdout).into_owned();
        if !cached_text.is_empty() {
            return Ok(cached_text);
        }
    }

    Ok(diff_text)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitBranchInfo {
    pub name: String,
    pub is_current: bool,
}

#[tauri::command]
pub async fn git_list_branches(cwd: String) -> AppResult<Vec<GitBranchInfo>> {
    let output = Command::new("git")
        .args(["branch", "--list", "--no-color"])
        .current_dir(&cwd)
        .output()
        .map_err(|e| AppError::Git(format!("Failed to run git branch: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Git(format!("git branch failed: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let branches = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let is_current = line.starts_with('*');
            let name = line.trim_start_matches('*').trim().to_string();
            GitBranchInfo { name, is_current }
        })
        .collect();

    Ok(branches)
}

#[tauri::command]
pub async fn git_checkout(cwd: String, branch: String) -> AppResult<()> {
    let output = Command::new("git")
        .args(["checkout", &branch])
        .current_dir(&cwd)
        .output()
        .map_err(|e| AppError::Git(format!("Failed to run git checkout: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Git(format!("git checkout failed: {stderr}")));
    }
    Ok(())
}

fn read_branch(cwd: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("Failed to run git rev-parse: {e}"))?;

    if !output.status.success() {
        return Err("Not a git repository".into());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
