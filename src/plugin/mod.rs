use anyhow::Result;
use std::{collections::HashSet, sync::Arc};

use crate::packet::NetworkPacket;

mod battery;
mod ping;
mod connectivity_report;
mod notification;

#[async_trait::async_trait]
pub trait KdeConnectPlugin: std::fmt::Debug + Send + Sync {
    async fn handle(&self, packet: NetworkPacket) -> Result<()>;
}

pub trait KdeConnectPluginMetadata {
    fn incomping_capabilities() -> Vec<String>;
    fn outgoing_capabilities() -> Vec<String>;
}

#[derive(Debug, Default)]
pub struct PluginRepository {
    plugins: Vec<(HashSet<String>, Arc<dyn KdeConnectPlugin>)>,
    pub incoming_caps: HashSet<String>,
    pub outgoing_caps: HashSet<String>,
}

impl PluginRepository {
    pub fn new() -> Self {
        let mut this = Self::default();

        this.register(ping::PingPlugin);
        this.register(connectivity_report::ConnectivityReportPlugin);
        this.register(notification::NotificationPlugin::new());
        this.register(battery::BatteryPlugin);

        this
    }

    pub fn register<P>(&mut self, plugin: P)
    where
        P: KdeConnectPlugin + KdeConnectPluginMetadata + 'static,
    {
        let in_caps = P::incomping_capabilities();
        let out_caps = P::outgoing_capabilities();

        self.incoming_caps.extend(in_caps.iter().cloned());
        self.outgoing_caps.extend(out_caps.into_iter());

        self.plugins
            .push((in_caps.into_iter().collect(), Arc::new(plugin)));
    }

    pub async fn handle_packet(&self, packet: NetworkPacket) -> Result<()> {
        let typ = packet.typ.as_str();

        for (in_caps, plguin) in &self.plugins {
            if in_caps.contains(typ) {
                plguin.handle(packet).await?;
                return Ok(());
            }
        }

        Err(anyhow::anyhow!("No plugin found for packet type {}", typ))
    }
}
