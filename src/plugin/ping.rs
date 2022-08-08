use crate::packet::NetworkPacket;
use anyhow::Result;

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

#[derive(Debug)]
pub struct PingPlugin;

#[async_trait::async_trait]
impl KdeConnectPlugin for PingPlugin {
    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        dbg!(packet);
        Ok(())
    }
}

impl KdeConnectPluginMetadata for PingPlugin {
    fn incomping_capabilities() -> Vec<String> {
        vec!["kdeconnect.ping".into()]
    }
    fn outgoing_capabilities() -> Vec<String> {
        vec!["kdeconnect.ping".into()]
    }
}
