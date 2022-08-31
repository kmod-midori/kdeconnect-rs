use std::{mem::MaybeUninit, sync::Arc};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tao::menu::{ContextMenu, MenuItemAttributes};
use tokio::sync::Mutex;
use windows::Win32::System::Power::GetSystemPowerStatus;

use crate::{
    context::AppContextRef, device::DeviceHandle, event::SystemEvent, packet::NetworkPacket,
};

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
    device: DeviceHandle,
}

impl BatteryPlugin {
    pub fn new(dev: DeviceHandle, ctx: AppContextRef) -> Self {
        Self {
            ctx,
            battery_status: Mutex::new(None),
            device: dev,
        }
    }

    pub async fn send_battery_status(&self) -> Result<()> {
        let power_status = unsafe {
            let mut power_status = MaybeUninit::uninit();
            GetSystemPowerStatus(power_status.as_mut_ptr()).ok()?;
            power_status.assume_init()
        };

        if power_status.ACLineStatus == 255 /* Unknown status */
            || power_status.BatteryFlag & 128 != 0 /* No system battery */
            || power_status.BatteryFlag == 255
        /* Unknown status—unable to read the battery flag information */
        {
            return Ok(());
        }

        let battery_status = BatteryReport {
            current_charge: power_status.BatteryLifePercent,
            is_charging: power_status.ACLineStatus == 1,
            threshold_event: power_status.SystemStatusFlag, /* 1 if battery saver is on */
        };

        self.device
            .send_packet(NetworkPacket::new(
                "kdeconnect.battery",
                battery_status.clone(),
            ))
            .await;

        Ok(())
    }
}

#[async_trait::async_trait]
impl KdeConnectPlugin for BatteryPlugin {
    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        match packet.typ.as_str() {
            "kdeconnect.battery" => {
                let report: BatteryReport = packet.into_body()?;
                *self.battery_status.lock().await = Some(report);
                self.ctx.update_tray().await;
            }
            "kdeconnect.battery.request" => {
                self.send_battery_status().await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn tray_menu(&self, menu: &mut ContextMenu) {
        let status = self.battery_status.lock().await;
        if let Some(x) = status.as_ref() {
            let text = format!(
                "Battery:\t\t\t  {}%{}",
                x.current_charge,
                if x.is_charging { "+" } else { "" }
            );
            menu.add_item(MenuItemAttributes::new(&text).with_enabled(false));
        }
    }

    async fn handle_event(self: Arc<Self>, event: SystemEvent) -> Result<()> {
        match event {
            SystemEvent::PowerStatusUpdated => {
                self.send_battery_status().await?;
            }
            _ => {}
        }
        Ok(())
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
