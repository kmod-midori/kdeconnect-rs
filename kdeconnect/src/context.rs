use crate::{config::Config, device::DeviceManagerHandle, CustomWindowEvent};
use anyhow::Result;
use once_cell::sync::OnceCell;
use std::{fmt::Debug, sync::Arc};
use tao::{event_loop::EventLoopProxy, global_shortcut::ShortcutManager};
use tokio::{
    net::{TcpStream, ToSocketAddrs},
    sync::Mutex,
};
use tokio_rustls::{client::TlsStream, TlsAcceptor, TlsConnector};

pub type AppContextRef = Arc<ApplicationContext>;

pub struct ApplicationContext {
    pub device_manager: DeviceManagerHandle,
    // pub plugin_repo: PluginRepository,
    pub config: Config,
    pub tls_acceptor: OnceCell<TlsAcceptor>,
    pub tls_connector: OnceCell<TlsConnector>,
    pub event_loop_proxy: EventLoopProxy<CustomWindowEvent>,
    pub hotkey_manager: Mutex<ShortcutManager>,
}

impl Debug for ApplicationContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApplicationContext").finish()
    }
}

impl ApplicationContext {
    pub async fn new(
        config: Config,
        event_loop_proxy: EventLoopProxy<CustomWindowEvent>,
        hotkey_manager: ShortcutManager,
    ) -> Result<Arc<Self>> {
        let (device_manager_actor, device_manager) = crate::device::DeviceManagerActor::new();
        // let plugin_repo = PluginRepository::new();

        let this = Arc::new(Self {
            device_manager,
            // plugin_repo,
            config,
            tls_acceptor: OnceCell::new(),
            tls_connector: OnceCell::new(),
            event_loop_proxy,
            hotkey_manager: Mutex::new(hotkey_manager),
        });

        device_manager_actor.run(this.clone());
        // this.plugin_repo.start(this.clone()).await?;

        Ok(this)
    }

    pub fn setup_tls(&self, acceptor: TlsAcceptor, connector: TlsConnector) {
        self.tls_acceptor.set(acceptor).ok();
        self.tls_connector.set(connector).ok();
    }

    pub fn tls_acceptor(&self) -> TlsAcceptor {
        self.tls_acceptor.get().unwrap().clone()
    }

    pub fn tls_connector(&self) -> TlsConnector {
        self.tls_connector.get().unwrap().clone()
    }

    pub async fn tls_connect(
        &self,
        addr: impl ToSocketAddrs,
    ) -> std::io::Result<TlsStream<TcpStream>> {
        let stream = tokio::net::TcpStream::connect(addr).await?;
        let peer = stream.peer_addr()?;
        let tls_stream = self
            .tls_connector()
            .connect(
                tokio_rustls::rustls::ServerName::IpAddress(peer.ip()),
                stream,
            )
            .await?;

        Ok(tls_stream)
    }

    pub async fn update_tray(&self) {
        self.device_manager.update_tray().await;
    }
}
