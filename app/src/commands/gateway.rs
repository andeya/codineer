#[allow(unused_imports)]
use crate::error::{AppError, AppResult};
use aineer_gateway::{GatewayConfig, GatewayServer, GatewayStatus};
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

    let config = GatewayConfig::default();
    let listen_addr = config.listen_addr.clone();
    let server = Arc::new(GatewayServer::new(config));
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
