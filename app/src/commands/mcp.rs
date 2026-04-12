use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerInfo {
    pub name: String,
    pub transport: String,
    pub running: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolCallRequest {
    pub server_name: String,
    pub tool_name: String,
    #[allow(dead_code)]
    pub arguments: serde_json::Value,
}

#[tauri::command]
pub async fn list_mcp_servers() -> AppResult<Vec<McpServerInfo>> {
    // Will be populated once McpServerManager is initialised at startup.
    // For now returns an empty list.
    Ok(Vec::new())
}

#[tauri::command]
pub async fn start_mcp_server(name: String) -> AppResult<()> {
    tracing::info!("start_mcp_server: {name}");
    Err(AppError::Mcp(format!(
        "MCP server '{name}' start not yet wired"
    )))
}

#[tauri::command]
pub async fn stop_mcp_server(name: String) -> AppResult<()> {
    tracing::info!("stop_mcp_server: {name}");
    Err(AppError::Mcp(format!(
        "MCP server '{name}' stop not yet wired"
    )))
}

#[tauri::command]
pub async fn call_mcp_tool(request: McpToolCallRequest) -> AppResult<serde_json::Value> {
    tracing::info!(
        "call_mcp_tool: server={}, tool={}",
        request.server_name,
        request.tool_name
    );
    Err(AppError::Mcp(format!(
        "MCP tool call '{}/{}' not yet wired",
        request.server_name, request.tool_name
    )))
}
