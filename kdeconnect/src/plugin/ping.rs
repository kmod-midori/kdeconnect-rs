use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{device::DeviceHandle, packet::NetworkPacket, utils};

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

#[derive(Debug, Deserialize, Serialize)]
struct PingPacket {
    message: Option<String>,
}

#[derive(Debug)]
pub struct PingPlugin {
    dev: DeviceHandle,
}

impl PingPlugin {
    pub fn new(dev: DeviceHandle) -> Self {
        PingPlugin {
            dev,
            // ctx,
        }
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
}

impl KdeConnectPluginMetadata for PingPlugin {
    fn incoming_capabilities() -> Vec<String> {
        vec!["kdeconnect.ping".into()]
    }
    fn outgoing_capabilities() -> Vec<String> {
        vec!["kdeconnect.ping".into()]
    }
}
