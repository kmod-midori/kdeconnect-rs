//! This plugin allows to control the system volume.

use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use windows_audio_manager::AudioManagerHandle;

use crate::{device::DeviceHandle, packet::NetworkPacket};

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

const PACKET_TYPE_SYSTEM_VOLUME: &str = "kdeconnect.systemvolume";
const PACKET_TYPE_SYSTEM_VOLUME_REQUEST: &str = "kdeconnect.systemvolume.request";

lazy_static::lazy_static! {
    static ref AUDIO_MANAGER: AudioManagerHandle = {
        windows_audio_manager::AudioManager::new()
    };
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemVolumeSink {
    name: String,
    description: String,
    muted: bool,
    volume: u8,
    max_volume: u8,
    enabled: bool,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum SystemVolumePacket {
    #[serde(rename_all = "camelCase")]
    SinkList { sink_list: Vec<SystemVolumeSink> },
    VolumeUpdate {
        name: String,
        volume: u8,
        muted: bool,
    },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RequestPacket {
    #[serde(rename_all = "camelCase")]
    RequestSinks { request_sinks: bool },
    #[serde(rename_all = "camelCase")]
    Command {
        name: String,
        volume: Option<u8>,
        muted: Option<bool>,
        enabled: Option<bool>,
    },
}

#[derive(Debug)]
pub struct SystemVolumePlugin {
    dev: DeviceHandle,
}

impl SystemVolumePlugin {
    pub fn new(dev: DeviceHandle) -> Self {
        SystemVolumePlugin { dev }
    }

    pub async fn send_sink_list(&self) -> Result<()> {
        let sinks = AUDIO_MANAGER.get_audio_sink_info().await?;
        let mut sink_list = Vec::with_capacity(sinks.len());

        for (_id, sink) in sinks {
            sink_list.push(SystemVolumeSink {
                name: sink.name,
                description: sink.description,
                muted: sink.is_muted,
                volume: sink.volume,
                max_volume: 100,
                enabled: sink.is_active,
            });
        }

        self.dev
            .send_packet(NetworkPacket::new(
                PACKET_TYPE_SYSTEM_VOLUME,
                SystemVolumePacket::SinkList { sink_list },
            ))
            .await;

        Ok(())
    }

    async fn send_volume_update(&self, name: String, volume: u8, muted: bool) {
        self.dev
            .send_packet(NetworkPacket::new(
                PACKET_TYPE_SYSTEM_VOLUME,
                SystemVolumePacket::VolumeUpdate {
                    name,
                    volume,
                    muted,
                },
            ))
            .await;
    }
}

#[async_trait::async_trait]
impl KdeConnectPlugin for SystemVolumePlugin {
    async fn start(self: Arc<Self>) -> Result<()> {
        let this = Arc::downgrade(&self);
        let mut notify_rx = AUDIO_MANAGER.subscribe_notification().await?;

        tokio::spawn(async move {
            while let Some(notification) = notify_rx.recv().await {
                if let Some(this) = this.upgrade() {
                    match notification {
                        windows_audio_manager::AudioNotification::SinkListUpdated => {
                            this.send_sink_list().await.ok();
                        }
                        windows_audio_manager::AudioNotification::VolumeUpdated {
                            id: _id,
                            name,
                            volume,
                            muted,
                        } => {
                            this.send_volume_update(name, volume, muted).await;
                        }
                    }
                } else {
                    // The plugin has been dropped, so we can stop listening for notifications.
                    break;
                }
            }
        });

        Ok(())
    }

    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        match packet.typ.as_str() {
            PACKET_TYPE_SYSTEM_VOLUME_REQUEST => {
                match packet.into_body::<RequestPacket>()? {
                    RequestPacket::RequestSinks { .. } => {
                        self.send_sink_list().await?;
                    }
                    RequestPacket::Command {
                        name,
                        volume,
                        muted,
                        enabled: _enabled,
                    } => {
                        let sinks = AUDIO_MANAGER.get_audio_sink_info().await?;

                        for (id, sink) in sinks {
                            if sink.name == name {
                                if let Some(volume) = volume {
                                    AUDIO_MANAGER.set_volume(&id, volume).await?;
                                }
                                if let Some(muted) = muted {
                                    AUDIO_MANAGER.set_muted(&id, muted).await?;
                                }
                                // if let Some(enabled) = enabled {
                                //     AUDIO_MANAGER.set_default_sink(id).await?;
                                // }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }
}

impl KdeConnectPluginMetadata for SystemVolumePlugin {
    fn incoming_capabilities() -> Vec<String> {
        vec![PACKET_TYPE_SYSTEM_VOLUME_REQUEST.into()]
    }
    fn outgoing_capabilities() -> Vec<String> {
        vec![PACKET_TYPE_SYSTEM_VOLUME.into()]
    }
}
