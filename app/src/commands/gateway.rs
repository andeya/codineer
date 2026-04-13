#[allow(unused_imports)]
use crate::error::{AppError, AppResult};
use aineer_gateway::{GatewayConfig, GatewayServer, GatewayStatus};
use aineer_webai::WebAiEngine;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ManagedGateway {
    inner: Arc<Mutex<Option<Arc<GatewayServer>>>>,
}

impl ManagedGateway {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayStatusInfo {
    pub running: bool,
    pub listen_addr: Option<String>,
    pub status: String,
}

#[tauri::command]
pub async fn start_gateway(
    state: tauri::State<'_, ManagedGateway>,
    settings_state: tauri::State<'_, super::settings::ManagedSettings>,
) -> AppResult<GatewayStatusInfo> {
    let mut guard = state.inner.lock().await;
    if let Some(ref server) = *guard {
        if server.status() == GatewayStatus::Running {
            return Ok(GatewayStatusInfo {
                running: true,
                listen_addr: None,
                status: "running".into(),
            });
        }
    }

    let config = if let Ok(merged) = settings_state.merged() {
        let gw = merged.gateway.unwrap_or_default();
        GatewayConfig {
            enabled: gw.enabled.unwrap_or(true),
            listen_addr: gw.listen_addr.unwrap_or_else(|| "127.0.0.1:8090".into()),
            default_model: gw.default_model,
        }
    } else {
        GatewayConfig::default()
    };
    let listen_addr = config.listen_addr.clone();
    let mut gateway = GatewayServer::new(config);

    if let Some(handle) = crate::app_handle() {
        gateway = gateway.with_webai(WebAiEngine::new(handle.clone()));
    }
    let server = Arc::new(gateway);
    let server_clone = Arc::clone(&server);
    tokio::spawn(async move {
        if let Err(e) = server_clone.start().await {
            tracing::error!("Gateway exited with error: {e}");
        }
    });

    let info = GatewayStatusInfo {
        running: true,
        listen_addr: Some(listen_addr),
        status: "starting".into(),
    };
    *guard = Some(server);
    Ok(info)
}

#[tauri::command]
pub async fn stop_gateway(state: tauri::State<'_, ManagedGateway>) -> AppResult<()> {
    let mut guard = state.inner.lock().await;
    *guard = None;
    Ok(())
}

#[tauri::command]
pub async fn get_gateway_status(
    state: tauri::State<'_, ManagedGateway>,
) -> AppResult<GatewayStatusInfo> {
    let guard = state.inner.lock().await;
    match &*guard {
        Some(server) => {
            let status = server.status();
            Ok(GatewayStatusInfo {
                running: status == GatewayStatus::Running || status == GatewayStatus::Starting,
                listen_addr: None,
                status: format!("{status:?}").to_lowercase(),
            })
        }
        None => Ok(GatewayStatusInfo {
            running: false,
            listen_addr: None,
            status: "stopped".into(),
        }),
    }
}
