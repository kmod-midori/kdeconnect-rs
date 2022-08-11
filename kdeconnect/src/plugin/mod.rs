use anyhow::Result;
use std::{collections::HashSet, sync::Arc};

use crate::{
    context::AppContextRef, device::DeviceHandle, event::KdeConnectEvent, packet::NetworkPacket,
    utils,
};

mod battery;
mod clipboard;
mod connectivity_report;
mod mpris;
mod ping;
mod receive_mouse;
mod receive_notifications;

#[async_trait::async_trait]
pub trait KdeConnectPlugin: std::fmt::Debug + Send + Sync {
    async fn start(self: Arc<Self>) -> Result<()> {
        Ok(())
    }
    async fn handle(&self, packet: NetworkPacket) -> Result<()>;
    async fn handle_event(self: Arc<Self>, _event: KdeConnectEvent) -> Result<()> {
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

lazy_static::lazy_static! {
    pub static ref ALL_CAPS: (Vec<String>, Vec<String>) = {
        let mut incoming_caps = vec![];
        let mut outgoing_caps = vec![];

        incoming_caps.extend(ping::PingPlugin::incoming_capabilities());
        outgoing_caps.extend(ping::PingPlugin::outgoing_capabilities());
        incoming_caps
            .extend(connectivity_report::ConnectivityReportPlugin::incoming_capabilities());
        outgoing_caps
            .extend(connectivity_report::ConnectivityReportPlugin::outgoing_capabilities());
        incoming_caps.extend(clipboard::ClipboardPlugin::incoming_capabilities());
        outgoing_caps.extend(clipboard::ClipboardPlugin::outgoing_capabilities());
        incoming_caps.extend(mpris::MprisPlugin::incoming_capabilities());
        outgoing_caps.extend(mpris::MprisPlugin::outgoing_capabilities());
        incoming_caps
            .extend(receive_notifications::ReceiveNotificationsPlugin::incoming_capabilities());
        outgoing_caps
            .extend(receive_notifications::ReceiveNotificationsPlugin::outgoing_capabilities());
        incoming_caps.extend(receive_mouse::ReceiveMousePlugin::incoming_capabilities());
        outgoing_caps.extend(receive_mouse::ReceiveMousePlugin::outgoing_capabilities());
        incoming_caps.extend(battery::BatteryPlugin::incoming_capabilities());
        outgoing_caps.extend(battery::BatteryPlugin::outgoing_capabilities());

        (incoming_caps, outgoing_caps)
    };
}

#[derive(Debug, Default)]
pub struct PluginRepository {
    plugins: Vec<(HashSet<String>, Arc<dyn KdeConnectPlugin>)>,
    pub incoming_caps: HashSet<String>,
    pub outgoing_caps: HashSet<String>,
}

impl PluginRepository {
    pub async fn new(dev: DeviceHandle, ctx: AppContextRef) -> Self {
        let mut this = Self::default();

        this.register(ping::PingPlugin);
        this.register(connectivity_report::ConnectivityReportPlugin);
        this.register(clipboard::ClipboardPlugin::new());
        utils::log_if_error(
            "Failed to initialize MPRIS plugin",
            mpris::MprisPlugin::new(dev.clone(), ctx.clone())
                .await
                .map(|p| this.register(p)),
        );
        this.register(receive_notifications::ReceiveNotificationsPlugin::new(
            dev.clone(),
            ctx.clone(),
        ));
        this.register(receive_mouse::ReceiveMousePlugin);
        this.register(battery::BatteryPlugin);

        // Start the plugins
        let plugins = this
            .plugins
            .iter()
            .map(|(_, p)| Arc::clone(p))
            .collect::<Vec<_>>();
        tokio::spawn(async move {
            for plugin in plugins {
                if let Err(e) = plugin.clone().start().await {
                    log::error!("Failed to start plugin {:?}: {:?}", plugin, e);
                }
            }
        });

        this
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

    pub async fn handle_packet(&self, packet: NetworkPacket) -> Result<()> {
        let typ = packet.typ.as_str();

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
            if let Err(e) = plugin.clone().handle_event(event.clone()).await {
                log::error!("Error handling event: {}", e);
            }
        }
    }
}
