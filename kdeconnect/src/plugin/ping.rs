use anyhow::Result;
use serde::{Deserialize, Serialize};
use winrt_toast::{Text, Toast, ToastManager};

use crate::{device::DeviceHandle, packet::NetworkPacket};

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

#[derive(Debug, Deserialize, Serialize)]
struct PingPacket {
    message: Option<String>,
}

#[derive(Debug)]
pub struct PingPlugin {
    dev: DeviceHandle,
    toast_manager: ToastManager,
}

impl PingPlugin {
    pub fn new(dev: DeviceHandle) -> Self {
        PingPlugin {
            dev,
            toast_manager: ToastManager::new(
                "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe",
            ),
            // ctx,
        }
    }
}

#[async_trait::async_trait]
impl KdeConnectPlugin for PingPlugin {
    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        let body: PingPacket = packet.into_body()?;

        let mut toast = Toast::new();
        toast
            .header(crate::utils::global_toast_header())
            .text1(body.message.unwrap_or_else(|| "Ping!".into()))
            .text3(
                Text::new(self.dev.device_id())
                    .with_placement(winrt_toast::text::TextPlacement::Attribution),
            );

        let manager = self.toast_manager.clone();
        tokio::task::spawn_blocking(move || manager.show(&toast, None, None, None)).await??;

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
