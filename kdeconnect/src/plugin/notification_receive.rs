/*!
This plugin listens to packages with type "kdeconnect.notification" that will
contain all the information of the other device notifications.

The other device will report us every notification that is created or dismissed,
so we can keep in sync a local list of notifications.

At the beginning we can request the already existing notifications by sending a
package with the boolean "request" set to true.

The received packages will contain the following fields:

"id" (string): A unique notification id.
"appName" (string): The app that generated the notification
"ticker" (string): The title or headline of the notification, for compatibility with older Android versions.
"isClearable" (boolean): True if we can request to dismiss the notification.
"isCancel" (boolean): True if the notification was dismissed in the peer device.
"requestAnswer" (boolean): True if this is an answer to a "request" package.
"title" (string): The title of the notification.
"text" (string): The text/content of the notification.
"requestReplyId" (string): Used to reply to messages.
"silent" (bool): Handle this notification silent, i.e. don't show a notification, but show it in the plasmoid.

Additionally the package can contain a payload with the icon of the notification
in PNG format. If there another field will be present:

"payloadHash" (string): MD5 hash of the payload. Used as a filename to store the payload.

The content of these fields is used to display the notifications to the user.
Note that if we receive a second notification with the same "id", the existing notification is updated.

If the user dismisses a notification from this device, we have to request the
other device to remove it. This is done by sending a package with the fields
"id" set to the id of the notification we want to dismiss and a boolean "cancel"
set to true. The other device will answer with a notification package with
"isCancel" set to true when it is dismissed.
 */
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{Context, Result};
use lru_cache::LruCache;
use serde::{Deserialize, Serialize};
use tao::menu::{ContextMenu, MenuId, MenuItemAttributes};
use tokio::sync::Mutex;
use winrt_toast::{DismissalReason, Header, Text, Toast};

use crate::{
    cache::PAYLOAD_CACHE, context::AppContextRef, device::DeviceHandle, event::SystemEvent,
    packet::NetworkPacket, utils,
};

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

const PACKET_TYPE_NOTIFICATION_REQUEST: &str = "kdeconnect.notification.request";

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

#[derive(Debug)]
pub struct NotificationReceivePlugin {
    ctx: AppContextRef,
    device: DeviceHandle,
    group_hash: String,
    id_to_icon_path: Mutex<LruCache<String, PathBuf>>,
    mute_menu_id: MenuId,
    muted: AtomicBool,
}

impl NotificationReceivePlugin {
    pub fn new(dev: DeviceHandle, ctx: AppContextRef) -> Self {
        Self {
            ctx,
            group_hash: format!(
                "{:x}",
                md5::compute(&format!("receive_notifications:{}", dev.device_id()))
            ),
            mute_menu_id: MenuId::new(&format!("{}:notifications:mute", dev.device_id())),
            muted: AtomicBool::new(false),
            id_to_icon_path: Mutex::new(LruCache::new(100)),
            device: dev,
        }
    }

    async fn show_notification(
        &self,
        notification: IncomingNotification,
        payload_info: Option<PayloadInfo>,
    ) -> Result<()> {
        let id_hash = format!("{:x}", md5::compute(&notification.id));
        let app_name_hash = format!("{:x}", md5::compute(&notification.app_name));

        let (title, text) =
            if let (Some(title), Some(text)) = (notification.title, notification.text) {
                (title, text)
            } else {
                return Ok(());
            };

        let icon_path = {
            let mut id_to_icon_path = self.id_to_icon_path.lock().await;

            if let Some(h) = notification.payload_hash {
                drop(id_to_icon_path);

                let name = format!("{}.png", h);

                let icon_path = if let Some(path) = PAYLOAD_CACHE.get_path(&name).await? {
                    Some(path)
                } else if let Some(payload_info) = payload_info {
                    let data = self
                        .device
                        .fetch_payload(payload_info.port, payload_info.size as usize)
                        .await?;

                    PAYLOAD_CACHE.put(&name, data).await?;
                    let path = PAYLOAD_CACHE.get_path(&name).await?.unwrap();

                    Some(path)
                } else {
                    None
                };

                if let Some(ref icon_path) = icon_path {
                    let mut id_to_icon_path = self.id_to_icon_path.lock().await;
                    id_to_icon_path.insert(notification.id.clone(), icon_path.clone());
                }

                icon_path
            } else {
                id_to_icon_path
                    .get_mut(&notification.id)
                    .map(|icon_path| icon_path.clone())
            }
        };

        let mut toast = Toast::new();
        toast
            .header(Header::new(
                &app_name_hash,
                &notification.app_name,
                "action=headerClick",
            ))
            .text1(title)
            .text2(text)
            .text3(Text::new(self.device.device_name()).as_attribution())
            .expires_in(Duration::from_secs(60 * 60 * 12))
            .tag(&id_hash)
            .group(&self.group_hash)
            .remote_id(&notification.id);

        if let Some(path) = icon_path {
            toast.image(
                1,
                winrt_toast::Image::new_local(path)?
                    .with_placement(winrt_toast::content::image::ImagePlacement::AppLogoOverride),
            );
        }

        let id = notification.id.clone();
        let dev = self.device.clone();
        let rt_handle = tokio::runtime::Handle::current();
        let on_dismissed = Box::new(move |reason| match reason {
            Ok(DismissalReason::UserCanceled) => {
                // Dismiss the remote notification
                let dev = dev.clone();
                let id = id.clone();

                let task = async move {
                    dev.send_packet(NetworkPacket::new(
                        PACKET_TYPE_NOTIFICATION_REQUEST,
                        serde_json::json!({
                            "cancel": id,
                        }),
                    ))
                    .await;
                };

                rt_handle.spawn(task);
            }
            Ok(_) => {}
            Err(e) => {
                log::error!("Failed to get dismissal reason: {:?}", e);
            }
        });

        let id = notification.id.clone();
        let on_failed = Box::new(move |e| {
            log::error!("Failed to show notification {}: {:?}", id, e);
        });

        let on_activated = Box::new(move |_arg| {});

        tokio::task::spawn_blocking(move || {
            utils::TOAST_MANAGER.show_with_callbacks(
                &toast,
                Some(on_activated),
                Some(on_dismissed),
                Some(on_failed),
            )
        })
        .await??;

        Ok(())
    }

    async fn remove_notification(&self, id: &str) -> Result<()> {
        let group_hash = self.group_hash.clone();
        let id_hash = format!("{:x}", md5::compute(id));

        tokio::task::spawn_blocking(move || {
            utils::TOAST_MANAGER.remove_grouped_tag(&group_hash, &id_hash)
        })
        .await??;

        Ok(())
    }

    fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }
}

struct PayloadInfo {
    size: u64,
    port: u16,
}

#[async_trait::async_trait]
impl KdeConnectPlugin for NotificationReceivePlugin {
    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        // Extract payload
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

        match body {
            NotificationBody::Cancelled { id, .. } => {
                log::info!("Cancelled {}", id);
                self.remove_notification(&id)
                    .await
                    .context("Remove notification")?;
            }
            NotificationBody::Posted(notif) => {
                if self.is_muted() {
                    log::info!("Posted {} (muted)", notif.id);
                } else {
                    log::info!("Posted {}", notif.id);
                    self.show_notification(notif, payload_info)
                        .await
                        .context("Show notification")?;
                }
            }
        }

        Ok(())
    }

    async fn start(self: Arc<Self>) -> Result<()> {
        // Request all remote notifications
        let dev = self.device.clone();

        tokio::spawn(async move {
            dev.send_packet(NetworkPacket::new(
                PACKET_TYPE_NOTIFICATION_REQUEST,
                serde_json::json!({
                    "request": true,
                }),
            ))
            .await;
        });

        Ok(())
    }

    async fn tray_menu(&self, menu: &mut ContextMenu) {
        let mut submenu = ContextMenu::new();
        submenu.add_item(
            MenuItemAttributes::new("Mute")
                .with_selected(self.is_muted())
                .with_id(self.mute_menu_id),
        );
        menu.add_submenu("Notifications", true, submenu);
    }

    async fn handle_event(self: Arc<Self>, event: SystemEvent) -> Result<()> {
        if event.is_menu_clicked(self.mute_menu_id) {
            self.muted.fetch_xor(true, Ordering::Relaxed);
            self.ctx.update_tray().await;
        }
        Ok(())
    }
}

impl KdeConnectPluginMetadata for NotificationReceivePlugin {
    fn incoming_capabilities() -> Vec<String> {
        vec!["kdeconnect.notification".into()]
    }
    fn outgoing_capabilities() -> Vec<String> {
        vec![
            PACKET_TYPE_NOTIFICATION_REQUEST.into(),
            "kdeconnect.notification.reply".into(),
        ]
    }
}
