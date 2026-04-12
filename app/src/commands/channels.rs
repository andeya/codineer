#[allow(unused_imports)]
use crate::error::{AppError, AppResult};
use aineer_channels::ChannelSource;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelAdapterInfo {
    pub name: String,
    pub source: String,
    pub connected: bool,
}

fn source_label(s: &ChannelSource) -> &'static str {
    match s {
        ChannelSource::Desktop => "desktop",
        ChannelSource::Feishu => "feishu",
        ChannelSource::WeChat => "wechat",
        ChannelSource::WhatsApp => "whatsapp",
        ChannelSource::Gateway => "gateway",
    }
}

#[tauri::command]
pub async fn list_channel_adapters() -> AppResult<Vec<ChannelAdapterInfo>> {
    // Desktop is always available; concrete external adapters will be registered
    // when their respective implementations are added.
    Ok(vec![ChannelAdapterInfo {
        name: "Desktop".into(),
        source: source_label(&ChannelSource::Desktop).into(),
        connected: true,
    }])
}
