use anyhow::Result;
use std::{collections::HashSet, sync::Arc};

use crate::{context::AppContextRef, packet::NetworkPacket, event::KdeConnectEvent};

mod battery;
mod clipboard;
mod connectivity_report;
mod mpris;
mod receive_notifications;
mod receive_mouse;
mod ping;

#[async_trait::async_trait]
pub trait KdeConnectPlugin: std::fmt::Debug + Send + Sync {
    async fn start(self: Arc<Self>, _ctx: AppContextRef) -> Result<()> {
        Ok(())
    }
    async fn handle(&self, packet: IncomingPacket) -> Result<()>;
    async fn handle_event(&self, _event: KdeConnectEvent) -> Result<()> {
        Ok(())
    }
    async fn hotkeys(&self) -> Vec<()> {
        vec![]
    }
}

pub trait KdeConnectPluginMetadata {
    fn incoming_capabilities() -> Vec<String>;
    fn outgoing_capabilities() -> Vec<String>;
}

#[derive(Debug, Clone)]
pub struct IncomingPacket {
    device_id: String,
    inner: NetworkPacket,
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
        this.register(clipboard::ClipboardPlugin::new());
        this.register(mpris::MprisPlugin::new());
        this.register(receive_notifications::ReceiveNotificationsPlugin::new());
        this.register(receive_mouse::ReceiveMousePlugin);
        this.register(battery::BatteryPlugin);

        this
    }

    pub async fn start(&self, ctx: AppContextRef) -> Result<()> {
        for (_, plugin) in &self.plugins {
            let ctx = ctx.clone();
            let plugin = plugin.clone();

            plugin.start(ctx).await?;
        }
        Ok(())
    }

    pub fn register<P>(&mut self, plugin: P)
    where
        P: KdeConnectPlugin + KdeConnectPluginMetadata + 'static,
    {
        let in_caps = P::incoming_capabilities();
        let out_caps = P::outgoing_capabilities();

        log::info!(
            "Registering plugin: {:?} with in={:?}, out={:?}",
            plugin,
            in_caps,
            out_caps
        );

        self.incoming_caps.extend(in_caps.iter().cloned());
        self.outgoing_caps.extend(out_caps.into_iter());

        self.plugins
            .push((in_caps.into_iter().collect(), Arc::new(plugin)));
    }

    pub async fn handle_packet(&self, device_id: String, packet: NetworkPacket) -> Result<()> {
        let packet = IncomingPacket { device_id, inner: packet };
        let typ = packet.inner.typ.as_str();

        log::debug!("Incoming packet: {:?}", packet);

        for (in_caps, plguin) in &self.plugins {
            if in_caps.contains(typ) {
                plguin.handle(packet).await?;
                return Ok(());
            }
        }

        Err(anyhow::anyhow!("No plugin found for packet type {}", typ))
    }

    pub async fn handle_event(&self, event: KdeConnectEvent) {
        for (_, plugin) in &self.plugins {
            if let Err(e) = plugin.handle_event(event.clone()).await {
                log::error!("Error handling event: {}", e);
            }
        }
    }
}
