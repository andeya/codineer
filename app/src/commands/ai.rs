use crate::error::{AppError, AppResult};
use aineer_webai::WebAiEngine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use tauri::Emitter;

use aineer_cli::desktop::{self, ChatHistoryTurn, ShellContextSnippet, StreamDelta};

use super::next_block_id;

type WebAiEngineState<'a> = tauri::State<'a, WebAiEngine>;

/// Active AI streams: `block_id` -> cancel flag (set by `stop_ai_stream`).
static AI_STREAM_ABORT: LazyLock<Mutex<HashMap<u64, Arc<AtomicBool>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Serialize, Deserialize)]
pub struct AiMessageRequest {
    pub message: String,
    pub model: Option<String>,
    /// Workspace / project root for settings discovery (defaults to current dir).
    #[serde(default)]
    pub cwd: Option<String>,
    /// Recent shell runs from the UI (command + output) for zero-shot context.
    #[serde(default)]
    pub shell_context: Vec<ShellContextSnippet>,
    /// Prior chat turns (user/assistant) for multi-turn desktop chat.
    #[serde(default)]
    pub chat_history: Vec<ChatHistoryTurn>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiStreamDelta {
    pub(crate) block_id: u64,
    pub(crate) delta: String,
    /// `"text"` for formal output, `"thinking"` for model reasoning, empty on done.
    pub(crate) kind: String,
    pub(crate) done: bool,
}

/// Send a user message to the configured provider and stream assistant text via `ai_stream_delta`.
#[tauri::command]
pub async fn send_ai_message(
    app: tauri::AppHandle,
    webai_engine: WebAiEngineState<'_>,
    settings_state: tauri::State<'_, super::settings::ManagedSettings>,
    request: AiMessageRequest,
) -> AppResult<u64> {
    let block_id = next_block_id();
    let cwd = super::workspace_cwd_from(request.cwd.as_deref());
    let message = request.message.clone();
    let model = request.model.clone();
    let shell_context = request.shell_context.clone();
    let chat_history = request.chat_history.clone();

    tracing::info!(
        "send_ai_message: block_id={block_id}, cwd={}, model={:?}",
        cwd.display(),
        model
    );

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut map = AI_STREAM_ABORT
            .lock()
            .map_err(|e| AppError::Ai(format!("stream registry lock poisoned: {e}")))?;
        map.insert(block_id, Arc::clone(&cancel));
    }

    if let Some((provider_name, specific_model)) =
        model.as_deref().and_then(WebAiEngine::parse_webai_model)
    {
        let provider_name = provider_name.to_string();
        let model_id = specific_model.unwrap_or("").to_string();
        let app_clone = app.clone();
        let engine = (*webai_engine).clone();

        if let Ok(merged) = settings_state.merged() {
            if let Some(secs) = merged.webai_idle_timeout {
                engine.set_idle_timeout_secs(secs as u64);
            }
            if let Some(secs) = merged.webai_page_load_timeout {
                engine.set_page_load_timeout_secs(secs as u64);
            }
        }

        tokio::spawn(async move {
            let emit_delta = |delta: &str, kind: &str, done: bool| {
                let _ = app_clone.emit(
                    "ai_stream_delta",
                    AiStreamDelta {
                        block_id,
                        delta: delta.to_string(),
                        kind: kind.to_string(),
                        done,
                    },
                );
            };

            tracing::info!(block_id, %provider_name, %model_id, "webai stream task started");
            match engine.send_raw(&provider_name, &model_id, &message).await {
                Ok(mut rx) => {
                    while let Some(chunk) = rx.recv().await {
                        if cancel.load(Ordering::Relaxed) {
                            tracing::info!(block_id, "webai stream cancelled by user");
                            break;
                        }
                        emit_delta(&chunk, "text", false);
                    }
                    emit_delta("", "", true);
                }
                Err(e) => {
                    tracing::warn!(block_id, %provider_name, %e, "webai send_raw failed");
                    emit_delta(&format!("**Error:** {e}"), "text", true);
                }
            }

            if let Ok(mut map) = AI_STREAM_ABORT.lock() {
                map.remove(&block_id);
            }
        });

        return Ok(block_id);
    }

    let app_clone = app.clone();
    // `stream_desktop_chat` uses `dyn Write` internally; its future is not `Send`, so run it on a
    // current-thread runtime inside `spawn_blocking` instead of `tokio::spawn`.
    tokio::task::spawn_blocking(move || {
        let emit_delta = |delta: &str, kind: &str, done: bool| {
            let _ = app_clone.emit(
                "ai_stream_delta",
                AiStreamDelta {
                    block_id,
                    delta: delta.to_string(),
                    kind: kind.to_string(),
                    done,
                },
            );
        };

        let rt_result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();

        let result = match rt_result {
            Ok(rt) => rt.block_on(async {
                desktop::stream_desktop_chat(
                    &cwd,
                    model.as_deref(),
                    &message,
                    &shell_context,
                    &chat_history,
                    Arc::clone(&cancel),
                    |d| match d {
                        StreamDelta::Text(t) => emit_delta(t, "text", false),
                        StreamDelta::Thinking(t) => emit_delta(t, "thinking", false),
                    },
                )
                .await
            }),
            Err(e) => Err(aineer_cli::desktop::DesktopStreamError::Cli(
                aineer_cli::error::CliError::Other(format!("tokio runtime: {e}")),
            )),
        };

        match result {
            Ok(_) => emit_delta("", "", true),
            Err(e) => emit_delta(&format!("**Error:** {e}"), "text", true),
        }

        if let Ok(mut map) = AI_STREAM_ABORT.lock() {
            map.remove(&block_id);
        }
    });

    Ok(block_id)
}

#[tauri::command]
pub async fn stop_ai_stream(block_id: u64) -> AppResult<()> {
    tracing::info!("stop_ai_stream: block_id={block_id}");
    let map = AI_STREAM_ABORT
        .lock()
        .map_err(|e| AppError::Ai(format!("stream registry lock poisoned: {e}")))?;
    if let Some(flag) = map.get(&block_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}
