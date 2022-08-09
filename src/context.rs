use crate::{config::Config, device::DeviceManagerHandle, plugin::PluginRepository};
use anyhow::Result;
use once_cell::sync::OnceCell;
use std::{fmt::Debug, sync::Arc};
use tokio_rustls::{TlsAcceptor, TlsConnector};

pub type AppContextRef = Arc<ApplicationContext>;

pub struct ApplicationContext {
    pub device_manager: DeviceManagerHandle,
    pub plugin_repo: PluginRepository,
    pub config: Config,
    pub tls_acceptor: OnceCell<TlsAcceptor>,
    pub tls_connector: OnceCell<TlsConnector>,
}

impl Debug for ApplicationContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApplicationContext").finish()
    }
}

impl ApplicationContext {
    pub async fn new(config: Config) -> Result<Arc<Self>> {
        let (device_manager_actor, device_manager) = crate::device::DeviceManagerActor::new();
        let plugin_repo = PluginRepository::new();

        let this = Arc::new(Self {
            device_manager,
            plugin_repo,
            config,
            tls_acceptor: OnceCell::new(),
            tls_connector: OnceCell::new(),
        });

        device_manager_actor.run();
        this.plugin_repo.start(this.clone()).await?;

        Ok(this)
    }

    pub fn setup_tls(&self, acceptor: TlsAcceptor, connector: TlsConnector) {
        self.tls_acceptor.set(acceptor).ok();
        self.tls_connector.set(connector).ok();
    }

    pub fn tls_acceptor(&self) -> TlsAcceptor {
        self.tls_acceptor.get().unwrap().clone()
    }
}
