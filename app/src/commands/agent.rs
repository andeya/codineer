use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use tauri::Emitter;

use aineer_cli::desktop::{self, ChatHistoryTurn, DesktopStreamDelta, ShellContextSnippet};

use super::ai::AiStreamDelta;
use super::next_block_id;

/// Active agent tasks: `block_id` -> cancel flag (set by `stop_agent`).
static AGENT_ABORT: LazyLock<Mutex<HashMap<u64, Arc<AtomicBool>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentRequest {
    pub goal: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub shell_context: Vec<ShellContextSnippet>,
    /// Prior chat turns (same shape as desktop chat) for multi-turn agent context.
    #[serde(default)]
    pub chat_history: Vec<ChatHistoryTurn>,
}

/// Run one agent turn (tools enabled, `PermissionMode::Allow` — GUI stdin prompts are not used).
/// Streams text/thinking via `ai_stream_delta` (same channel as Chat) then `done: true`.
#[tauri::command]
pub async fn start_agent(app: tauri::AppHandle, request: AgentRequest) -> AppResult<u64> {
    let block_id = next_block_id();
    let cwd = super::workspace_cwd_from(request.cwd.as_deref());
    let goal = request.goal.clone();
    let model = request.model.clone();
    let shell_context = request.shell_context.clone();
    let chat_history = request.chat_history.clone();

    tracing::info!(
        "start_agent: block_id={block_id}, cwd={}, goal_len={}",
        cwd.display(),
        goal.len()
    );

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut map = AGENT_ABORT
            .lock()
            .map_err(|e| AppError::Agent(format!("agent registry lock poisoned: {e}")))?;
        map.insert(block_id, Arc::clone(&cancel));
    }

    let app_clone = app.clone();
    tokio::task::spawn_blocking(move || {
        let app_emit = app_clone.clone();
        let bid = block_id;
        let cancel_turn = Arc::clone(&cancel);

        let result = desktop::run_desktop_agent_turn(
            &cwd,
            model.as_deref(),
            &goal,
            &shell_context,
            &chat_history,
            cancel_turn,
            Box::new(move |d: DesktopStreamDelta| {
                let (kind, delta) = match d {
                    DesktopStreamDelta::Text(s) => ("text", s),
                    DesktopStreamDelta::Thinking(s) => ("thinking", s),
                };
                let _ = app_emit.emit(
                    "ai_stream_delta",
                    AiStreamDelta {
                        block_id: bid,
                        delta,
                        kind: kind.to_string(),
                        done: false,
                    },
                );
            }),
        );

        match result {
            Ok(_) => {
                let _ = app_clone.emit(
                    "ai_stream_delta",
                    AiStreamDelta {
                        block_id: bid,
                        delta: String::new(),
                        kind: String::new(),
                        done: true,
                    },
                );
            }
            Err(e) => {
                let _ = app_clone.emit(
                    "ai_stream_delta",
                    AiStreamDelta {
                        block_id: bid,
                        delta: format!("**Error:** {e}"),
                        kind: "text".into(),
                        done: true,
                    },
                );
            }
        }

        if let Ok(mut map) = AGENT_ABORT.lock() {
            map.remove(&block_id);
        }
    });

    Ok(block_id)
}

#[tauri::command]
pub async fn approve_tool(block_id: u64) -> AppResult<()> {
    tracing::info!(block_id, "approve_tool (no-op until GUI approval is wired)");
    Ok(())
}

#[tauri::command]
pub async fn deny_tool(block_id: u64) -> AppResult<()> {
    tracing::info!(block_id, "deny_tool (no-op until GUI approval is wired)");
    Ok(())
}

#[tauri::command]
pub async fn stop_agent(block_id: u64) -> AppResult<()> {
    tracing::info!("stop_agent: block_id={block_id}");
    let map = AGENT_ABORT
        .lock()
        .map_err(|e| AppError::Agent(format!("agent registry lock poisoned: {e}")))?;
    if let Some(flag) = map.get(&block_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}
