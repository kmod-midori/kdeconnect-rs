use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::{
    device::DeviceHandle,
    event::KdeConnectEvent,
    packet::NetworkPacket,
    utils::{self, clipboard::ClipboardContent},
};

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

const PACKET_TYPE_CLIPBOARD: &str = "kdeconnect.clipboard";
const PACKET_TYPE_CLIPBOARD_CONNECT: &str = "kdeconnect.clipboard.connect";

#[derive(Debug)]
struct CurrentClipboardContent {
    content: ClipboardContent,
    ts: u64,
}

impl CurrentClipboardContent {
    pub fn new_now(content: ClipboardContent) -> Self {
        Self {
            content,
            ts: utils::unix_ts_ms(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ClipboardPacket {
    content: String,
}

#[derive(Debug)]
pub struct ClipboardPlugin {
    content: Mutex<Option<CurrentClipboardContent>>,
    device: DeviceHandle,
}

impl ClipboardPlugin {
    pub fn new(dev: DeviceHandle) -> Self {
        Self {
            content: Mutex::new(None),
            device: dev,
        }
    }

    async fn read_clipboard(&self) -> Result<()> {
        let content = tokio::task::spawn_blocking(utils::clipboard::read).await??;

        let mut c = self.content.lock().await;
        *c = Some(CurrentClipboardContent::new_now(content));

        Ok(())
    }

    async fn write_clipboard(&self, text: impl Into<String>) -> Result<()> {
        let text = text.into();

        tokio::task::spawn_blocking(move || utils::clipboard::write(ClipboardContent::Text(text)))
            .await??;

        Ok(())
    }

    async fn send_clipboard(&self) {
        let content = self.content.lock().await;
        if let Some(content) = content.as_ref() {
            match &content.content {
                ClipboardContent::Text(s) => {
                    let packet = NetworkPacket::new(
                        PACKET_TYPE_CLIPBOARD,
                        ClipboardPacket { content: s.clone() },
                    );
                    self.device.send_packet(packet).await;
                }
                ClipboardContent::Files(_) => {}
                ClipboardContent::Unsupported => {}
            }
        }
    }
}

#[async_trait::async_trait]
impl KdeConnectPlugin for ClipboardPlugin {
    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        match packet.typ.as_str() {
            PACKET_TYPE_CLIPBOARD => {
                let body: ClipboardPacket = packet.into_body()?;
                self.write_clipboard(body.content)
                    .await
                    .context("Write clipboard")?;
            }
            PACKET_TYPE_CLIPBOARD_CONNECT => {}
            _ => {}
        }
        Ok(())
    }

    async fn handle_event(self: Arc<Self>, event: KdeConnectEvent) -> Result<()> {
        match event {
            KdeConnectEvent::ClipboardUpdated => {
                self.read_clipboard().await.context("Read clipboard")?;
                // self.send_clipboard().await;
            }
            _ => {}
        }
        Ok(())
    }
}

impl KdeConnectPluginMetadata for ClipboardPlugin {
    fn incoming_capabilities() -> Vec<String> {
        vec![
            PACKET_TYPE_CLIPBOARD.into(),
            PACKET_TYPE_CLIPBOARD_CONNECT.into(),
        ]
    }
    fn outgoing_capabilities() -> Vec<String> {
        vec![
            PACKET_TYPE_CLIPBOARD.into(),
            PACKET_TYPE_CLIPBOARD_CONNECT.into(),
        ]
    }
}
