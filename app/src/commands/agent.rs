#[allow(unused_imports)]
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::Emitter;

use aineer_engine::{ContentBlock, MessageRole, Session};

static NEXT_AGENT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentRequest {
    pub goal: String,
    pub context_block_ids: Vec<u64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentEvent {
    block_id: u64,
    kind: String,
    data: String,
}

/// Start an autonomous agent session.
///
/// The agent uses the engine's ConversationRuntime with tool execution enabled.
/// Events (text deltas, tool-use requests, approvals) are pushed to the frontend
/// via Tauri events.
#[tauri::command]
pub async fn start_agent(
    app: tauri::AppHandle,
    request: AgentRequest,
) -> AppResult<u64> {
    let block_id = NEXT_AGENT_ID.fetch_add(1, Ordering::Relaxed);

    tracing::info!("start_agent: block_id={block_id}, goal={}", request.goal);

    let mut session = Session {
        version: 1,
        messages: Vec::new(),
        cwd: None,
        model_id: None,
        created_at: Some(chrono::Utc::now().to_rfc3339()),
    };

    session.messages.push(aineer_engine::ConversationMessage {
        role: MessageRole::User,
        blocks: vec![ContentBlock::Text {
            text: request.goal.clone(),
        }],
        usage: None,
    });

    let app_clone = app.clone();
    tokio::spawn(async move {
        // TODO: Wire up ConversationRuntime with a real ToolExecutor and ApiClient.
        let _ = app_clone.emit(
            "agent_event",
            AgentEvent {
                block_id,
                kind: "text".into(),
                data: format!(
                    "Agent mode is not yet connected to the engine runtime. \
                     Goal: `{}`. Configure an API key in Settings → Models to enable.",
                    request.goal
                ),
            },
        );
        let _ = app_clone.emit(
            "agent_event",
            AgentEvent {
                block_id,
                kind: "done".into(),
                data: String::new(),
            },
        );
    });

    Ok(block_id)
}

#[tauri::command]
pub async fn approve_tool(block_id: u64) -> AppResult<()> {
    tracing::info!("approve_tool: block_id={block_id}");
    // TODO: Signal the running agent to proceed with the pending tool call.
    Ok(())
}

#[tauri::command]
pub async fn deny_tool(block_id: u64) -> AppResult<()> {
    tracing::info!("deny_tool: block_id={block_id}");
    // TODO: Signal the running agent to skip the pending tool call.
    Ok(())
}

#[tauri::command]
pub async fn stop_agent() -> AppResult<()> {
    tracing::info!("stop_agent");
    // TODO: Cancel the running agent.
    Ok(())
}
