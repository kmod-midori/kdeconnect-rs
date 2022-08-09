use crate::packet::NetworkPacket;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

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
        payload_hash: Option<String>,
        ticker: Option<String>,
        title: Option<String>,
        text: Option<String>,
    },
}

#[derive(Debug)]
pub struct NotificationPlugin {
    id_cache: Mutex<lru_cache::LruCache<String, ()>>,
}

impl NotificationPlugin {
    pub fn new() -> Self {
        Self {
            id_cache: Mutex::new(lru_cache::LruCache::new(100)),
        }
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
                id,
                time,
                title,
                text,
                app_name,
                ..
            } => {
                let mut id_cache = self.id_cache.lock().await;
                let id_key = format!("{}|{}", id, time);
                if id_cache.contains_key(&id_key) {
                    return Ok(());
                }

                if let (Some(title), Some(text)) = (title, text) {
                    let _ = notify_rust::Notification::new()
                        .summary(&format!("{}: {}", app_name, title))
                        .body(&text)
                        .show();
                }

                id_cache.insert(id_key, ());
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
