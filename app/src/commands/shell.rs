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

#[tauri::command]
pub async fn shell_complete(partial: String, cwd: Option<String>) -> AppResult<Vec<String>> {
    let result = tokio::task::spawn_blocking(move || complete_impl(&partial, cwd.as_deref()))
        .await
        .map_err(|e| AppError::Shell(format!("Task join error: {e}")))?;
    result.map_err(AppError::Shell)
}

fn complete_impl(partial: &str, cwd: Option<&str>) -> Result<Vec<String>, String> {
    if partial.is_empty() {
        return Ok(Vec::new());
    }

    let script = if cfg!(target_os = "windows") {
        return Ok(Vec::new());
    } else {
        format!(
            "compgen -A command -A file -A directory -- {} 2>/dev/null | head -20",
            shell_escape(partial)
        )
    };

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let mut cmd = std::process::Command::new(&shell);
    cmd.args(["-c", &script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());

    if let Some(dir) = cwd {
        let path = std::path::Path::new(dir);
        if path.is_dir() {
            cmd.current_dir(path);
        }
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run compgen: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let completions: Vec<String> = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();

    Ok(completions)
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
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

    let result = tokio::task::spawn_blocking({
        move || wait_with_timeout(&mut child, timeout)
    })
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
