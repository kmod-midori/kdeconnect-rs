use std::{collections::HashSet, sync::Arc};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::{device::DeviceHandle, event::KdeConnectEvent, packet::NetworkPacket, utils};
use clipboard_win::{formats, Clipboard, Getter, Setter};

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

const PACKET_TYPE_CLIPBOARD: &str = "kdeconnect.clipboard";
const PACKET_TYPE_CLIPBOARD_CONNECT: &str = "kdeconnect.clipboard.connect";

fn try_open_clipboard() -> Result<Clipboard> {
    let mut clipboard = None;
    for _ in 0..10 {
        if let Ok(c) = Clipboard::new() {
            clipboard = Some(c);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    if let Some(c) = clipboard {
        Ok(c)
    } else {
        Err(anyhow::anyhow!("Could not open clipboard"))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ClipboardPacket {
    content: String,
}

#[derive(Debug)]
pub struct ClipboardPlugin {
    content: Mutex<Option<ClipboardContent>>,
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
        let content = tokio::task::spawn_blocking(|| {
            let _clip = try_open_clipboard()?;

            let formats = clipboard_win::EnumFormats::new().collect::<HashSet<_>>();

            if formats.contains(&formats::CF_UNICODETEXT) {
                let mut text = String::new();
                formats::Unicode.read_clipboard(&mut text)?;
                return Ok(ClipboardContent::new_now(ClipboardContentType::Text(text)));
            }

            if formats.contains(&formats::CF_HDROP) {
                let mut list: Vec<String> = vec![];
                formats::FileList.read_clipboard(&mut list)?;
                return Ok(ClipboardContent::new_now(ClipboardContentType::Files(list)));
            }

            Ok::<_, anyhow::Error>(ClipboardContent::new_now(ClipboardContentType::Unsupported))
        })
        .await??;

        let mut c = self.content.lock().await;
        *c = Some(content);

        Ok(())
    }

    async fn write_clipboard(&self, text: impl Into<String>) -> Result<()> {
        let text = text.into();

        tokio::task::spawn_blocking(move || {
            let _clip = try_open_clipboard()?;

            formats::Unicode.write_clipboard(&text)?;

            Ok::<_, anyhow::Error>(())
        })
        .await??;

        Ok(())
    }

    async fn send_clipboard(&self) {
        let content = self.content.lock().await;
        if let Some(content) = content.as_ref() {
            match &content.content {
                ClipboardContentType::Text(s) => {
                    let packet = NetworkPacket::new(
                        PACKET_TYPE_CLIPBOARD,
                        ClipboardPacket {
                            content: s.clone(),
                        },
                    );
                    self.device.send_packet(packet).await;
                }
                ClipboardContentType::Files(_) => {}
                ClipboardContentType::Unsupported => {}
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

#[derive(Debug)]
struct ClipboardContent {
    content: ClipboardContentType,
    ts: u64,
}

impl ClipboardContent {
    fn new_now(content: ClipboardContentType) -> Self {
        Self {
            content,
            ts: utils::unix_ts_ms(),
        }
    }
}

#[derive(Debug)]
enum ClipboardContentType {
    Text(String),
    Files(Vec<String>),
    Unsupported,
}
