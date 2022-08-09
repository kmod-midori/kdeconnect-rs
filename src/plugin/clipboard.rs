use anyhow::Result;

use super::{KdeConnectPlugin, KdeConnectPluginMetadata, IncomingPacket};

#[derive(Debug)]
pub struct ClipboardPlugin;

#[async_trait::async_trait]
impl KdeConnectPlugin for ClipboardPlugin {
    async fn handle(&self, packet: IncomingPacket) -> Result<()> {
        dbg!(packet);
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
