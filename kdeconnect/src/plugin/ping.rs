use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tao::menu::{ContextMenu, MenuId, MenuItemAttributes};

use crate::{device::DeviceHandle, event::SystemEvent, packet::NetworkPacket, utils};

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

const PACKET_TYPE_PING: &str = "kdeconnect.ping";

#[derive(Debug, Deserialize, Serialize)]
struct PingPacket {
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Debug)]
pub struct PingPlugin {
    dev: DeviceHandle,
    menu_id: MenuId,
}

impl PingPlugin {
    pub fn new(dev: DeviceHandle) -> Self {
        PingPlugin {
            menu_id: MenuId::new(&format!("{}:ping", dev.device_id())),
            dev,
        }
    }

    pub async fn send_ping(&self) {
        self.dev
            .send_packet(NetworkPacket::new(
                PACKET_TYPE_PING,
                PingPacket { message: None },
            ))
            .await;
    }
}

#[async_trait::async_trait]
impl KdeConnectPlugin for PingPlugin {
    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        let body: PingPacket = packet.into_body()?;

        utils::simple_toast(
            "Ping",
            body.message.as_deref(),
            Some(self.dev.device_name()),
        )
        .await;

        Ok(())
    }

    async fn tray_menu(&self, menu: &mut ContextMenu) {
        menu.add_item(MenuItemAttributes::new("Ping").with_id(self.menu_id));
    }

    async fn handle_event(self: Arc<Self>, event: SystemEvent) -> Result<()> {
        if event.is_menu_clicked(self.menu_id) {
            self.send_ping().await;
        }
        Ok(())
    }
}

impl KdeConnectPluginMetadata for PingPlugin {
    fn incoming_capabilities() -> Vec<String> {
        vec!["kdeconnect.ping".into()]
    }
    fn outgoing_capabilities() -> Vec<String> {
        vec!["kdeconnect.ping".into()]
    }
}
