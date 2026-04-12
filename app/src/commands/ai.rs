#[allow(unused_imports)]
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::Emitter;

use aineer_engine::{ContentBlock, MessageRole, Session};

static NEXT_BLOCK_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Serialize, Deserialize)]
pub struct AiMessageRequest {
    pub message: String,
    pub model: Option<String>,
    pub context_block_ids: Vec<u64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiStreamDelta {
    block_id: u64,
    delta: String,
    done: bool,
}

/// Send a user message to the AI engine and stream the response back via Tauri events.
///
/// Currently creates a session but relies on a configured API provider to actually
/// produce responses.  When no provider is configured, returns a helpful message.
#[tauri::command]
pub async fn send_ai_message(
    app: tauri::AppHandle,
    request: AiMessageRequest,
) -> AppResult<u64> {
    let block_id = NEXT_BLOCK_ID.fetch_add(1, Ordering::Relaxed);
    let model = request.model.clone().unwrap_or_else(|| "default".into());

    tracing::info!("send_ai_message: block_id={block_id}, model={model}");

    let mut session = Session {
        version: 1,
        messages: Vec::new(),
        cwd: None,
        model_id: Some(model.clone()),
        created_at: Some(chrono::Utc::now().to_rfc3339()),
    };

    session.messages.push(aineer_engine::ConversationMessage {
        role: MessageRole::User,
        blocks: vec![ContentBlock::Text {
            text: request.message,
        }],
        usage: None,
    });

    // Spawn the streaming task so the IPC call returns immediately with the block_id.
    let app_clone = app.clone();
    tokio::spawn(async move {
        // TODO: Wire up ProviderClient + ConversationRuntime for real streaming.
        // For now, emit a placeholder delta so the frontend shows something.
        let _ = app_clone.emit(
            "ai_stream_delta",
            AiStreamDelta {
                block_id,
                delta: format!(
                    "AI streaming is not yet connected to a provider. \
                     Model requested: `{model}`. \
                     Configure an API key in Settings → Models to enable AI chat."
                ),
                done: true,
            },
        );
    });

    Ok(block_id)
}

#[tauri::command]
pub async fn stop_ai_stream(block_id: u64) -> AppResult<()> {
    tracing::info!("stop_ai_stream: block_id={block_id}");
    // TODO: Cancel the running stream associated with block_id.
    Ok(())
}
