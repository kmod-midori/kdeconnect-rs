use anyhow::Result;
use std::sync::Arc;
use tokio::sync::oneshot;

use crate::packet::{NetworkPacket, NetworkPacketWithPayload};

use super::{DeviceManagerHandle, Message};

#[derive(Clone)]
pub struct DeviceHandle {
    pub(super) device_id: Arc<String>,
    pub(super) device_name: Arc<String>,
    pub(super) manager_handle: DeviceManagerHandle,
}

impl std::fmt::Debug for DeviceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceHandle")
            .field("device_id", &self.device_id)
            .finish()
    }
}

impl DeviceHandle {
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Send packet to device
    pub async fn send_packet(&self, packet: impl Into<NetworkPacketWithPayload>) {
        self.manager_handle
            .send_packet(self.device_id(), packet)
            .await;
    }

    /// Dispatch received packet from the device to plugins
    pub async fn dispatch_packet(&self, packet: impl Into<NetworkPacket>) {
        self.manager_handle
            .send_message(Message::Packet {
                device_id: self.device_id.to_string(),
                packet: packet.into(),
            })
            .await;
    }

    pub async fn fetch_payload(&self, port: u16, size: usize) -> Result<Vec<u8>> {
        let (tx, rx) = oneshot::channel();

        self.manager_handle
            .send_message(Message::FetchPayload {
                device_id: self.device_id.to_string(),
                port,
                size,
                reply: tx,
            })
            .await;

        rx.await?
    }
}
