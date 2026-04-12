use crate::error::{AppError, AppResult};
use crate::pty_manager::PtyManager;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SpawnPtyRequest {
    pub shell: Option<String>,
    /// When set, spawns `shell -c command` instead of an interactive shell.
    /// The PTY exits when the command finishes — used for queued command execution.
    pub command: Option<String>,
    pub cwd: Option<String>,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PtyId {
    pub id: u64,
}

#[tauri::command]
pub async fn spawn_pty(
    state: tauri::State<'_, PtyManager>,
    app: tauri::AppHandle,
    request: SpawnPtyRequest,
) -> AppResult<PtyId> {
    let id = state
        .spawn(
            app,
            request.shell,
            request.command,
            request.cwd,
            request.cols,
            request.rows,
        )
        .map_err(AppError::Shell)?;
    Ok(PtyId { id })
}

#[tauri::command]
pub async fn write_pty(
    state: tauri::State<'_, PtyManager>,
    id: u64,
    data: Vec<u8>,
) -> AppResult<()> {
    state.write(id, &data).map_err(AppError::Shell)
}

#[tauri::command]
pub async fn resize_pty(
    state: tauri::State<'_, PtyManager>,
    id: u64,
    cols: u16,
    rows: u16,
) -> AppResult<()> {
    state.resize(id, cols, rows).map_err(AppError::Shell)
}

#[tauri::command]
pub async fn kill_pty(state: tauri::State<'_, PtyManager>, id: u64) -> AppResult<()> {
    state.kill(id).map_err(AppError::Shell)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteCommandRequest {
    pub command: String,
    pub cwd: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub timed_out: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItem {
    pub value: String,
    pub is_dir: bool,
}

#[tauri::command]
pub async fn shell_complete(
    partial: String,
    cwd: Option<String>,
    is_first_word: Option<bool>,
) -> AppResult<Vec<CompletionItem>> {
    let first = is_first_word.unwrap_or(false);
    let result =
        tokio::task::spawn_blocking(move || complete_impl(&partial, cwd.as_deref(), first))
            .await
            .map_err(|e| AppError::Shell(format!("Task join error: {e}")))?;
    Ok(result)
}

fn complete_impl(partial: &str, cwd: Option<&str>, is_first_word: bool) -> Vec<CompletionItem> {
    let base = cwd
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });

    let mut items = complete_files(partial, &base);

    if is_first_word && !partial.contains('/') {
        let mut cmds = complete_commands(partial);
        let mut hist = complete_from_history(partial);
        // Order: history first, then commands, then files
        hist.append(&mut cmds);
        hist.append(&mut items);
        items = hist;
    }

    // Deduplicate by value, preserving order
    let mut seen = std::collections::HashSet::new();
    items.retain(|item| seen.insert(item.value.clone()));
    items.truncate(30);
    items
}

/// Complete file/directory names relative to `base`.
fn complete_files(partial: &str, base: &std::path::Path) -> Vec<CompletionItem> {
    let (dir_part, prefix) = match partial.rfind('/') {
        Some(idx) => (&partial[..=idx], &partial[idx + 1..]),
        None => ("", partial),
    };

    let search_dir = if dir_part.is_empty() {
        base.to_path_buf()
    } else {
        base.join(dir_part)
    };

    let entries = match std::fs::read_dir(&search_dir) {
        Ok(rd) => rd,
        Err(_) => return Vec::new(),
    };

    let prefix_lower = prefix.to_lowercase();
    let mut dirs: Vec<CompletionItem> = Vec::new();
    let mut files: Vec<CompletionItem> = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden files unless user typed a dot prefix
        if name_str.starts_with('.') && !prefix.starts_with('.') {
            continue;
        }

        if !name_str.to_lowercase().starts_with(&prefix_lower) {
            continue;
        }

        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        let value = format!("{dir_part}{name_str}");

        let item = CompletionItem { value, is_dir };
        if is_dir {
            dirs.push(item);
        } else {
            files.push(item);
        }
    }

    dirs.sort_by(|a, b| a.value.cmp(&b.value));
    files.sort_by(|a, b| a.value.cmp(&b.value));
    dirs.append(&mut files);
    dirs
}

/// Complete executable command names by scanning `$PATH` directories.
fn complete_commands(partial: &str) -> Vec<CompletionItem> {
    if partial.is_empty() {
        return Vec::new();
    }

    let path_var = std::env::var("PATH").unwrap_or_default();
    let sep = if cfg!(target_os = "windows") { ';' } else { ':' };
    let prefix_lower = partial.to_lowercase();

    let mut seen = std::collections::HashSet::new();
    let mut items = Vec::new();

    for dir in path_var.split(sep) {
        let entries = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.to_lowercase().starts_with(&prefix_lower) {
                continue;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = entry.metadata() {
                    if meta.permissions().mode() & 0o111 == 0 {
                        continue;
                    }
                }
            }
            if seen.insert(name_str.to_string()) {
                items.push(CompletionItem {
                    value: name_str.into_owned(),
                    is_dir: false,
                });
            }
        }
    }

    items.sort_by(|a, b| a.value.cmp(&b.value));
    items
}

/// Extract unique command names from shell history whose first word starts
/// with `partial`. Most-recently-used commands come first (up to 10).
fn complete_from_history(partial: &str) -> Vec<CompletionItem> {
    if partial.is_empty() {
        return Vec::new();
    }

    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from);
    let home = match home {
        Some(h) => h,
        None => return Vec::new(),
    };

    let shell = std::env::var("SHELL").unwrap_or_default();
    let hist_path = if shell.ends_with("/zsh") {
        home.join(".zsh_history")
    } else {
        home.join(".bash_history")
    };

    let content = match std::fs::read_to_string(&hist_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let prefix_lower = partial.to_lowercase();
    let mut seen = std::collections::HashSet::new();
    let mut items = Vec::new();

    // Read from the end (most recent first)
    for raw_line in content.lines().rev() {
        // zsh extended history format: `: <timestamp>:<duration>;<command>`
        let line = if raw_line.starts_with(": ") {
            raw_line.split_once(';').map_or("", |(_, cmd)| cmd).trim()
        } else {
            raw_line.trim()
        };

        if line.is_empty() {
            continue;
        }

        let first_word = line.split_whitespace().next().unwrap_or("");
        if !first_word.to_lowercase().starts_with(&prefix_lower) {
            continue;
        }

        // Only return the command name for consistent completion behavior
        if seen.insert(first_word.to_string()) {
            items.push(CompletionItem {
                value: first_word.to_string(),
                is_dir: false,
            });
        }
        if items.len() >= 10 {
            break;
        }
    }

    items
}

const DEFAULT_TIMEOUT_MS: u64 = 30_000;

#[tauri::command]
pub async fn execute_command(request: ExecuteCommandRequest) -> AppResult<CommandOutput> {
    let start = std::time::Instant::now();
    let timeout =
        std::time::Duration::from_millis(request.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS));

    let (shell, flag) = if cfg!(target_os = "windows") {
        ("cmd".to_string(), "/C")
    } else {
        (
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string()),
            "-c",
        )
    };

    let mut cmd = std::process::Command::new(&shell);
    cmd.arg(flag)
        .arg(&request.command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    if let Some(ref cwd) = request.cwd {
        let path = std::path::Path::new(cwd);
        if path.is_dir() {
            cmd.current_dir(path);
        }
    }

    cmd.env("TERM", "xterm-256color");

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::Shell(format!("Failed to spawn command: {e}")))?;

    let result = tokio::task::spawn_blocking(move || wait_with_timeout(&mut child, timeout))
        .await
        .map_err(|e| AppError::Shell(format!("Task join error: {e}")))?;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        WaitResult::Completed(output) => Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code().unwrap_or(-1),
            duration_ms,
            timed_out: false,
        }),
        WaitResult::TimedOut(partial_stdout, partial_stderr) => Ok(CommandOutput {
            stdout: partial_stdout,
            stderr: format!(
                "{}[Command timed out after {}s and was killed]",
                if partial_stderr.is_empty() {
                    String::new()
                } else {
                    format!("{partial_stderr}\n")
                },
                timeout.as_secs(),
            ),
            exit_code: 124,
            duration_ms,
            timed_out: true,
        }),
        WaitResult::Error(msg) => Err(AppError::Shell(msg)),
    }
}

enum WaitResult {
    Completed(std::process::Output),
    TimedOut(String, String),
    Error(String),
}

fn wait_with_timeout(child: &mut std::process::Child, timeout: std::time::Duration) -> WaitResult {
    let deadline = std::time::Instant::now() + timeout;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child
                    .stdout
                    .take()
                    .map(|mut r| {
                        let mut buf = Vec::new();
                        std::io::Read::read_to_end(&mut r, &mut buf).ok();
                        buf
                    })
                    .unwrap_or_default();
                let stderr = child
                    .stderr
                    .take()
                    .map(|mut r| {
                        let mut buf = Vec::new();
                        std::io::Read::read_to_end(&mut r, &mut buf).ok();
                        buf
                    })
                    .unwrap_or_default();

                return WaitResult::Completed(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return WaitResult::TimedOut(String::new(), String::new());
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => return WaitResult::Error(format!("Failed to wait for process: {e}")),
        }
    }
}
