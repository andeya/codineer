use crate::error::AppResult;
use aineer_gateway::{GatewayConfig, GatewayServer, GatewayStatus};
use aineer_webai::WebAiEngine;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

type WebAiEngineState<'a> = tauri::State<'a, WebAiEngine>;

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
    webai_engine: WebAiEngineState<'_>,
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

    // start_gateway is an explicit user action, so always force enabled=true
    // regardless of what the persisted config says (it may not have been
    // flushed yet when this command fires).
    let config = if let Ok(merged) = settings_state.merged() {
        let gw = merged.gateway.unwrap_or_default();
        GatewayConfig {
            enabled: true,
            listen_addr: gw.listen_addr.unwrap_or_else(|| "127.0.0.1:8090".into()),
            default_model: gw.default_model,
        }
    } else {
        GatewayConfig {
            enabled: true,
            ..GatewayConfig::default()
        }
    };
    let listen_addr = config.listen_addr.clone();
    let mut gateway = GatewayServer::new(config);
    gateway = gateway.with_webai((*webai_engine).clone());
    let server = Arc::new(gateway);
    server.mark_starting();
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
