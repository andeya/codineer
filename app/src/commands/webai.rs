use crate::error::{AppError, AppResult};
use aineer_webai::WebAiEngine;
use serde::Serialize;
use tauri::Emitter;

type WebAiEngineState<'a> = tauri::State<'a, WebAiEngine>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAiProviderInfo {
    pub id: String,
    pub name: String,
    pub models: Vec<WebAiModelInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAiModelInfo {
    pub id: String,
    pub name: String,
    pub default: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAiPageStatus {
    pub provider_id: String,
    pub provider_name: String,
    pub active: bool,
}

#[tauri::command]
pub async fn webai_list_providers(
    engine: WebAiEngineState<'_>,
) -> AppResult<Vec<WebAiProviderInfo>> {
    let providers = engine
        .list_providers()
        .into_iter()
        .map(|p| {
            let models = engine
                .list_models(&p.id)
                .into_iter()
                .map(|m| WebAiModelInfo {
                    id: m.id,
                    name: m.name,
                    default: m.default,
                })
                .collect();
            WebAiProviderInfo {
                id: p.id,
                name: p.name,
                models,
            }
        })
        .collect();
    Ok(providers)
}

#[tauri::command]
pub async fn webai_start_auth(
    engine: WebAiEngineState<'_>,
    provider_id: String,
) -> AppResult<String> {
    let handle =
        crate::app_handle().ok_or_else(|| AppError::Gateway("app handle not available".into()))?;
    let providers = engine.list_providers();
    let config = providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| AppError::Gateway(format!("unknown provider: {provider_id}")))?;

    let creds = aineer_webai::webauth::start_webauth(handle, config)
        .await
        .map_err(|e| AppError::Gateway(format!("webauth failed: {e}")))?;

    Ok(creds.provider_id)
}

#[tauri::command]
pub async fn webai_list_authenticated() -> AppResult<Vec<String>> {
    Ok(aineer_webai::webauth::list_authenticated())
}

#[tauri::command]
pub async fn webai_logout(provider_id: String) -> AppResult<()> {
    aineer_webai::webauth::logout(&provider_id)
        .map_err(|e| AppError::Gateway(format!("logout failed: {e}")))?;
    if let Some(handle) = crate::app_handle() {
        let _ = handle.emit("webai-auth-changed", &provider_id);
    }
    Ok(())
}

#[tauri::command]
pub async fn webai_list_pages(
    engine: WebAiEngineState<'_>,
    settings_state: tauri::State<'_, super::settings::ManagedSettings>,
) -> AppResult<Vec<WebAiPageStatus>> {
    if let Ok(merged) = settings_state.merged() {
        if let Some(secs) = merged.webai_idle_timeout {
            engine.set_idle_timeout_secs(secs as u64);
        }
    }
    let active_ids = engine.list_active_pages().await;
    let providers = engine.list_providers();
    let result = providers
        .into_iter()
        .map(|p| WebAiPageStatus {
            active: active_ids.contains(&p.id),
            provider_id: p.id,
            provider_name: p.name,
        })
        .collect();
    Ok(result)
}

#[tauri::command]
pub async fn webai_close_page(engine: WebAiEngineState<'_>, provider_id: String) -> AppResult<()> {
    engine.close_page(&provider_id).await;
    Ok(())
}

#[tauri::command]
pub async fn webai_close_all_pages(engine: WebAiEngineState<'_>) -> AppResult<()> {
    engine.close_all_pages().await;
    Ok(())
}
