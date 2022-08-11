use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use windows::{
    core::{Interface, HSTRING},
    Data::Xml::Dom::XmlDocument,
    Foundation::{PropertyValue, TypedEventHandler},
    Globalization::Calendar,
    UI::Notifications::{
        ToastDismissalReason, ToastDismissedEventArgs, ToastFailedEventArgs, ToastNotification,
        ToastNotificationManager,
    },
};

use crate::{
    cache::PAYLOAD_CACHE, context::AppContextRef, device::DeviceHandle, packet::NetworkPacket,
};

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

/// Convert a string to a HSTRING
fn hs(s: impl AsRef<str>) -> HSTRING {
    let s = s.as_ref();
    HSTRING::from(s)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum NotificationBody {
    #[serde(rename_all = "camelCase")]
    Cancelled { id: String, is_cancel: bool },
    #[serde(rename_all = "camelCase")]
    Posted(IncomingNotification),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct IncomingNotification {
    id: String,
    only_once: bool,
    is_clearable: bool,
    app_name: String,
    time: String, // long
    payload_hash: Option<String>,
    ticker: Option<String>,
    title: Option<String>,
    text: Option<String>,
}

lazy_static::lazy_static! {
    static ref APP_ID: HSTRING = {
        hs("{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe")
    };
}

#[derive(Debug)]
pub struct ReceiveNotificationsPlugin {
    device: DeviceHandle,
    group_hash: HSTRING,
}

impl ReceiveNotificationsPlugin {
    pub fn new(dev: DeviceHandle, _ctx: AppContextRef) -> Self {
        Self {
            group_hash: hs(format!(
                "{:x}",
                md5::compute(&format!("receive_notifications:{}", dev.device_id()))
            )),
            device: dev,
        }
    }

    async fn show_notification(
        &self,
        notification: IncomingNotification,
        payload_info: Option<PayloadInfo>,
    ) -> Result<()> {
        let group_hash = self.group_hash.clone();
        let id_hash = hs(format!("{:x}", md5::compute(&notification.id)));

        let (title, text) =
            if let (Some(title), Some(text)) = (notification.title, notification.text) {
                (format!("{}: {}", notification.app_name, title), text)
            } else {
                return Ok(());
            };

        let icon_url = if let Some(h) = notification.payload_hash {
            let name = format!("{}.png", h);

            if let Some(path) = PAYLOAD_CACHE.get_path(&name).await? {
                Some(url::Url::from_file_path(path).unwrap().to_string())
            } else {
                if let Some(payload_info) = payload_info {
                    let data = self
                        .device
                        .fetch_payload(payload_info.port, payload_info.size as usize)
                        .await?;
                    PAYLOAD_CACHE.put(&name, data).await?;
                    let path = PAYLOAD_CACHE.get_path(&name).await?.unwrap();
                    Some(url::Url::from_file_path(path).unwrap().to_string())
                } else {
                    None
                }
            }
        } else {
            None
        };

        tokio::task::spawn_blocking(move || {
            let toast_doc = XmlDocument::new()?;

            let toast_el = toast_doc.CreateElement(&hs("toast"))?;
            toast_doc.AppendChild(&toast_el)?;
            let visual_el = toast_doc.CreateElement(&hs("visual"))?;
            toast_el.AppendChild(&visual_el)?;

            let binding_el = toast_doc.CreateElement(&hs("binding"))?;
            visual_el.AppendChild(&binding_el)?;
            binding_el.SetAttribute(&hs("template"), &hs("ToastGeneric"))?;
            // Title
            let text_el = toast_doc.CreateElement(&hs("text"))?;
            binding_el.AppendChild(&text_el)?;
            text_el.SetInnerText(&hs(title))?;
            text_el.SetAttribute(&hs("id"), &hs("1"))?;
            // Text
            let text2_el = toast_doc.CreateElement(&hs("text"))?;
            binding_el.AppendChild(&text2_el)?;
            text2_el.SetInnerText(&hs(text))?;
            text2_el.SetAttribute(&hs("id"), &hs("2"))?;
            if let Some(url) = icon_url {
                let image_el = toast_doc.CreateElement(&hs("image"))?;
                binding_el.AppendChild(&image_el)?;
                image_el.SetAttribute(&hs("placement"), &hs("appLogoOverride"))?;
                image_el.SetAttribute(&hs("src"), &hs(url))?;
            }

            let actions_el = toast_doc.CreateElement(&hs("actions"))?;
            toast_el.AppendChild(&actions_el)?;
            let action_el = toast_doc.CreateElement(&hs("action"))?;
            actions_el.AppendChild(&action_el)?;
            action_el.SetAttribute(&hs("content"), &hs("Cancel"))?;
            action_el.SetAttribute(&hs("arguments"), &hs("cancel"))?;

            let toast = ToastNotification::CreateToastNotification(&toast_doc)?;
            toast.Failed(&TypedEventHandler::new(
                |_, args: &Option<ToastFailedEventArgs>| {
                    if let Some(args) = args {
                        if let Err(e) = args.ErrorCode().and_then(|e| e.ok()) {
                            log::error!("Failed to show notification: {:?}", e);
                        }
                    }
                    Ok(())
                },
            ))?;
            let id = notification.id.clone();
            toast.Dismissed(&TypedEventHandler::new(
                move |_, args: &Option<ToastDismissedEventArgs>| {
                    let args = if let Some(args) = args {
                        args
                    } else {
                        return Ok(());
                    };

                    match args.Reason() {
                        Ok(ToastDismissalReason::UserCanceled) => {
                            log::info!("Notification {} cancelled by user", id);
                        }
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("Failed to get dismissal reason: {:?}", e);
                        }
                    }

                    Ok(())
                },
            ))?;

            let now = Calendar::new()?;
            now.AddHours(12)?;
            let dt = now.GetDateTime()?;
            toast.SetExpirationTime(&PropertyValue::CreateDateTime(dt)?.cast()?)?;

            toast.SetRemoteId(&hs(notification.id))?;
            toast.SetGroup(&group_hash)?;
            toast.SetTag(&id_hash)?;

            let notifier = ToastNotificationManager::CreateToastNotifierWithId(&APP_ID)?;
            notifier.Show(&toast)?;

            Ok::<_, anyhow::Error>(())
        })
        .await??;

        Ok(())
    }

    async fn remove_notification(&self, id: &str) -> Result<()> {
        let group_hash = self.group_hash.clone();
        let id_hash = hs(format!("{:x}", md5::compute(id)));

        tokio::task::spawn_blocking(move || {
            ToastNotificationManager::History()?.RemoveGroupedTagWithId(
                &id_hash,
                &group_hash,
                &APP_ID,
            )?;
            Ok::<_, anyhow::Error>(())
        });

        Ok(())
    }
}

struct PayloadInfo {
    size: u64,
    port: u16,
}

#[async_trait::async_trait]
impl KdeConnectPlugin for ReceiveNotificationsPlugin {
    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        let payload_info = if let (Some(size), Some(pi)) = (
            packet.payload_size.as_ref(),
            packet.payload_transfer_info.as_ref(),
        ) {
            Some(PayloadInfo {
                size: *size,
                port: pi.port,
            })
        } else {
            None
        };
        let body: NotificationBody = packet.into_body()?;

        log::info!("Notification: {:?}", body);

        match body {
            NotificationBody::Cancelled { id, .. } => {
                self.remove_notification(&id)
                    .await
                    .context("Remove notification")?;
            }
            NotificationBody::Posted(notif) => {
                self.show_notification(notif, payload_info)
                    .await
                    .context("Show notification")?;
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
