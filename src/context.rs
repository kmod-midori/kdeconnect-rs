use std::sync::Arc;

use crate::{device::DeviceManagerHandle, plugin::PluginRepository};

pub type AppContextRef = Arc<ApplicationContext>;

#[derive(Debug)]
pub struct ApplicationContext {
    pub device_manager: DeviceManagerHandle,
    pub plugin_repo: PluginRepository,
}

impl ApplicationContext {
    pub async fn new() -> Arc<Self> {
        let (device_manager_actor, device_manager) = crate::device::DeviceManagerActor::new();
        let plugin_repo = PluginRepository::new();

        let this = Arc::new(Self {
            device_manager,
            plugin_repo
        });

        device_manager_actor.run();
        this.plugin_repo.start(this.clone()).await;

        this
    }
}
