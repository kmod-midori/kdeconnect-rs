use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use windows::{
    core::HSTRING,
    Data::Xml::Dom::XmlDocument,
    Foundation::TypedEventHandler,
    UI::Notifications::{ToastNotification, ToastNotificationManager},
};

use super::{IncomingPacket, KdeConnectPlugin, KdeConnectPluginMetadata};

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
pub struct ReceiveNotificationsPlugin {
    id_cache: Mutex<lru_cache::LruCache<String, ()>>,
}

mod toast {
    struct Toast {}
}

impl ReceiveNotificationsPlugin {
    pub fn new() -> Self {
        Self {
            id_cache: Mutex::new(lru_cache::LruCache::new(100)),
        }
    }

    async fn show_notification(&self) -> Result<()> {
        let app_id = HSTRING::from(
            "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe",
        );
        let doc = HSTRING::from(
            r#"
        <toast>
            <visual>
                <binding template="ToastGeneric">
                    <text>Hello World!</text>
                </binding>
            </visual>
            <actions>
                <action content="check" arguments="check" />
                <action content="cancel" arguments="cancel" />
            </actions>
        </toast>
        "#,
        );

        let toast_xml = XmlDocument::new()?;
        toast_xml.LoadXml(&doc)?;

        let toast = ToastNotification::CreateToastNotification(&toast_xml)?;
        toast.Activated(&TypedEventHandler::new(|_, _| {
            log::info!("Activated");
            Ok(())
        }))?;
        toast.Failed(&TypedEventHandler::new(|_, _| {
            log::info!("Failed");
            Ok(())
        }))?;

        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&app_id)?;
        notifier.Show(&toast)?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl KdeConnectPlugin for ReceiveNotificationsPlugin {
    async fn handle(&self, packet: IncomingPacket) -> Result<()> {
        let body: NotificationBody = packet.inner.into_body()?;

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
                // let mut id_cache = self.id_cache.lock().await;
                // let id_key = format!("{}|{}", id, time);
                // if id_cache.contains_key(&id_key) {
                //     return Ok(());
                // }

                // if let (Some(title), Some(text)) = (title, text) {
                //     let _ = notify_rust::Notification::new()
                //         .summary(&format!("{}: {}", app_name, title))
                //         .body(&text)
                //         .show();
                // }

                if let Err(e) = self.show_notification().await {
                    log::error!("Failed to show notification: {:?}", e);
                }

                // id_cache.insert(id_key, ());
            }
        }

        Ok(())
    }
}

impl KdeConnectPluginMetadata for ReceiveNotificationsPlugin {
    fn incoming_capabilities() -> Vec<String> {
        vec!["kdeconnect.notification".into()]
    }
    fn outgoing_capabilities() -> Vec<String> {
        vec![
            "kdeconnect.notification.request".into(),
            "kdeconnect.notification.reply".into(),
        ]
    }
}
