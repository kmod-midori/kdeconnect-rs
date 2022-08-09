use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{IncomingPacket, KdeConnectPlugin, KdeConnectPluginMetadata};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatteryReport {
    /// Battery level in percent
    current_charge: u8,
    is_charging: bool,
    /// 1 if battery is low, 0 if not.
    threshold_event: u8,
}

#[derive(Debug)]
pub struct BatteryPlugin;

#[async_trait::async_trait]
impl KdeConnectPlugin for BatteryPlugin {
    async fn handle(&self, packet: IncomingPacket) -> Result<()> {
        match packet.inner.typ.as_str() {
            "kdeconnect.battery" => {
                let report: BatteryReport = packet.inner.into_body()?;
                log::info!("Battery report: {:?}", report);
            }
            "kdeconnect.battery.request" => {
                // ignore
            }
            _ => {}
        }
        Ok(())
    }
}

impl KdeConnectPluginMetadata for BatteryPlugin {
    fn incomping_capabilities() -> Vec<String> {
        vec![
            "kdeconnect.battery".into(),
            "kdeconnect.battery.request".into(),
        ]
    }
    fn outgoing_capabilities() -> Vec<String> {
        vec![
            "kdeconnect.battery".into(),
            "kdeconnect.battery.request".into(),
        ]
    }
}
