use crate::packet::NetworkPacket;
use anyhow::Result;

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

#[derive(Debug)]
pub struct ClipboardPlugin;

#[async_trait::async_trait]
impl KdeConnectPlugin for ClipboardPlugin {
    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        dbg!(packet);
        Ok(())
    }
}

impl KdeConnectPluginMetadata for ClipboardPlugin {
    fn incomping_capabilities() -> Vec<String> {
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
