use anyhow::Result;
use serde::{Deserialize, Serialize};
use tao::menu::{ContextMenu, MenuItemAttributes};
use tokio::sync::Mutex;

use crate::{context::AppContextRef, packet::NetworkPacket};

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

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
pub struct BatteryPlugin {
    ctx: AppContextRef,
    battery_status: Mutex<Option<BatteryReport>>,
}

impl BatteryPlugin {
    pub fn new(ctx: AppContextRef) -> Self {
        Self {
            ctx,
            battery_status: Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl KdeConnectPlugin for BatteryPlugin {
    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        match packet.typ.as_str() {
            "kdeconnect.battery" => {
                let report: BatteryReport = packet.into_body()?;
                *self.battery_status.lock().await = Some(report);
                self.ctx.update_tray_menu().await;
            }
            "kdeconnect.battery.request" => {
                // ignore
            }
            _ => {}
        }
        Ok(())
    }

    async fn tray_menu(&self, menu: &mut ContextMenu) {
        let status = self.battery_status.lock().await;
        if let Some(x) = status.as_ref() {
            let text = format!(
                "Battery: {}%{}",
                x.current_charge,
                if x.is_charging { "+" } else { "" }
            );
            menu.add_item(MenuItemAttributes::new(&text).with_enabled(false));
        }
    }
}

impl KdeConnectPluginMetadata for BatteryPlugin {
    fn incoming_capabilities() -> Vec<String> {
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
