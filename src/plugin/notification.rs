use crate::packet::NetworkPacket;
use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum NotificationBody {
    #[serde(rename_all = "camelCase")]
    Cancelled { id: String, is_cancel: bool },
    #[serde(rename_all = "camelCase")]
    Posted {
        id: String,
        only_once: bool,
        is_clearable: bool,
        app_name: String,
        time: String, // long
        ticker: Option<String>,
        title: Option<String>,
        text: Option<String>,
    },
}

#[derive(Debug)]
pub struct NotificationPlugin {}

impl NotificationPlugin {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl KdeConnectPlugin for NotificationPlugin {
    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        let body: NotificationBody = packet.into_body()?;

        log::info!("Notification: {:?}", body);

        match body {
            NotificationBody::Cancelled { .. } => {}
            NotificationBody::Posted {
                title,
                text,
                app_name,
                ..
            } => {
                if let (Some(title), Some(text)) = (title, text) {
                    let _ = notify_rust::Notification::new()
                        .appname(&format!("KDE Connect - {}", app_name))
                        .summary(&title)
                        .body(&text)
                        .show();
                }
            }
        }

        Ok(())
    }
}

impl KdeConnectPluginMetadata for NotificationPlugin {
    fn incomping_capabilities() -> Vec<String> {
        vec!["kdeconnect.notification".into()]
    }
    fn outgoing_capabilities() -> Vec<String> {
        vec![
            "kdeconnect.notification.request".into(),
            "kdeconnect.notification.reply".into(),
        ]
    }
}
