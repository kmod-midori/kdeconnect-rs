use std::sync::Arc;

use crate::{device::DeviceManagerHandle, plugin::PluginRepository};

pub type AppContextRef = Arc<ApplicationContext>;

pub struct ApplicationContext {
    pub device_manager: DeviceManagerHandle,
    pub plugin_repo: PluginRepository,
}

impl ApplicationContext {
    pub fn new() -> Arc<Self> {
        let (device_manager_actor, device_manager) = crate::device::DeviceManagerActor::new();
        let plugin_repo = PluginRepository::new();

        let this = Arc::new(Self {
            device_manager,
            plugin_repo
        });

        device_manager_actor.run();

        this
    }
}
