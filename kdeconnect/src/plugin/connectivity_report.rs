use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::packet::NetworkPacket;

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SignalStrength {
    network_type: String,
    signal_strength: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectivityReport {
    signal_strengths: HashMap<String, SignalStrength>,
}

#[derive(Debug)]
pub struct ConnectivityReportPlugin;

#[async_trait::async_trait]
impl KdeConnectPlugin for ConnectivityReportPlugin {
    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        match packet.typ.as_str() {
            "kdeconnect.connectivity_report" => {
                let strengths: ConnectivityReport = packet.into_body()?;
                log::info!("Connectivity report: {:?}", strengths);
            }
            "kdeconnect.connectivity_report.request" => {
                // ignore
            }
            _ => {}
        }

        Ok(())
    }
}

impl KdeConnectPluginMetadata for ConnectivityReportPlugin {
    fn incoming_capabilities() -> Vec<String> {
        vec![
            "kdeconnect.connectivity_report".into(),
            "kdeconnect.connectivity_report.request".into(),
        ]
    }
    fn outgoing_capabilities() -> Vec<String> {
        vec!["kdeconnect.connectivity_report.request".into()]
    }
}
