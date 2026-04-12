#[allow(unused_imports)]
use crate::error::{AppError, AppResult};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LspDiagnosticItem {
    pub file: String,
    pub line: u32,
    pub character: u32,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LspHoverInfo {
    pub contents: String,
    pub range_start_line: u32,
    pub range_end_line: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LspCompletionItem {
    pub label: String,
    pub kind: Option<String>,
    pub detail: Option<String>,
}

#[tauri::command]
pub async fn lsp_diagnostics(path: String) -> AppResult<Vec<LspDiagnosticItem>> {
    tracing::info!("lsp_diagnostics: {path}");
    // LspManager is used internally by the engine; direct IPC diagnostics
    // will be wired once the manager is integrated as managed state.
    Ok(Vec::new())
}

#[tauri::command]
pub async fn lsp_hover(
    path: String,
    line: u32,
    character: u32,
) -> AppResult<Option<LspHoverInfo>> {
    tracing::info!("lsp_hover: {path}:{line}:{character}");
    Ok(None)
}

#[tauri::command]
pub async fn lsp_completions(
    path: String,
    line: u32,
    character: u32,
) -> AppResult<Vec<LspCompletionItem>> {
    tracing::info!("lsp_completions: {path}:{line}:{character}");
    Ok(Vec::new())
}
