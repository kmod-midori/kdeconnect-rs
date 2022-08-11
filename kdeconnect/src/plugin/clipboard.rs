use std::{collections::HashSet, sync::Arc};

use anyhow::Result;

use crate::{event::KdeConnectEvent, utils, packet::NetworkPacket};
use clipboard_win::{formats, Clipboard, Getter};

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

#[derive(Debug)]
pub struct ClipboardPlugin {}

impl ClipboardPlugin {
    pub fn new() -> Self {
        Self {}
    }

    async fn read_clipboard(&self) -> Result<()> {
        let content = tokio::task::spawn_blocking(|| {
            let mut clipboard = None;
            for _ in 0..10 {
                if let Ok(c) = Clipboard::new() {
                    clipboard = Some(c);
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            let _clip = if let Some(c) = clipboard {
                c
            } else {
                return Err(anyhow::anyhow!("Could not open clipboard"));
            };

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

        // dbg!(content);

        Ok(())
    }
}

#[async_trait::async_trait]
impl KdeConnectPlugin for ClipboardPlugin {
    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        dbg!(packet);
        Ok(())
    }

    async fn handle_event(self: Arc<Self>, event: KdeConnectEvent) -> Result<()> {
        match event {
            KdeConnectEvent::ClipboardUpdated => {
                // log::info!("Clipboard updated");
                if let Err(e) = self.read_clipboard().await {
                    log::error!("Error reading clipboard: {}", e);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl KdeConnectPluginMetadata for ClipboardPlugin {
    fn incoming_capabilities() -> Vec<String> {
        vec![
            "kdeconnect.clipboard".into(),
            "kdeconnect.clipboard.connect".into(),
        ]
    }
    fn outgoing_capabilities() -> Vec<String> {
        vec![
            "kdeconnect.clipboard".into(),
            "kdeconnect.clipboard.connect".into(),
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
