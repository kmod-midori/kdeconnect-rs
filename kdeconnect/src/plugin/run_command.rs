use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{device::DeviceHandle, packet::NetworkPacket, utils};

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

const PACKET_TYPE_RUNCOMMAND: &str = "kdeconnect.runcommand";
const PACKET_TYPE_RUNCOMMAND_REQUEST: &str = "kdeconnect.runcommand.request";

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum RunCommandRequestPacket {
    #[serde(rename_all = "camelCase")]
    RequestCommandList { request_command_list: bool },
    #[serde(rename_all = "camelCase")]
    Setup { setup: bool },
    #[serde(rename_all = "camelCase")]
    RunCommand { key: String },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunCommandPacket {
    /// A JSON string containing a map of keys to commands
    command_list: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Command {
    name: String,
    command: String,
}

#[derive(Debug)]
pub struct RunCommandPlugin {
    dev: DeviceHandle,
}

impl RunCommandPlugin {
    pub fn new(dev: DeviceHandle) -> Self {
        RunCommandPlugin {
            dev,
            // ctx,
        }
    }

    async fn send_command_list(&self) -> Result<()> {
        let mut command_list = HashMap::new();
        command_list.insert(
            "test".to_string(),
            Command {
                name: "Test".to_string(),
                command: "echo \"Hello World\"".to_string(),
            },
        );
        command_list.insert(
            "test2".to_string(),
            Command {
                name: "Test2".to_string(),
                command: "echo \"Hello World2\"".to_string(),
            },
        );
        let command_list = serde_json::to_string(&command_list)?;
        self.dev
            .send_packet(NetworkPacket::new(
                PACKET_TYPE_RUNCOMMAND,
                RunCommandPacket { command_list },
            ))
            .await;

        Ok(())
    }
}

#[async_trait::async_trait]
impl KdeConnectPlugin for RunCommandPlugin {
    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        match packet.typ.as_str() {
            PACKET_TYPE_RUNCOMMAND => {
                // TODO
            }
            PACKET_TYPE_RUNCOMMAND_REQUEST => {
                let body: RunCommandRequestPacket = packet.into_body()?;

                match body {
                    RunCommandRequestPacket::RequestCommandList { .. } => {
                        self.send_command_list().await?;
                    }
                    RunCommandRequestPacket::Setup { .. } => {
                        self.send_command_list().await?;
                    }
                    RunCommandRequestPacket::RunCommand { key } => {
                        log::info!("Received command with key: {}", key);
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }
}

impl KdeConnectPluginMetadata for RunCommandPlugin {
    fn incoming_capabilities() -> Vec<String> {
        vec![
            PACKET_TYPE_RUNCOMMAND.into(),
            PACKET_TYPE_RUNCOMMAND_REQUEST.into(),
        ]
    }
    fn outgoing_capabilities() -> Vec<String> {
        vec![
            PACKET_TYPE_RUNCOMMAND.into(),
            PACKET_TYPE_RUNCOMMAND_REQUEST.into(),
        ]
    }
}
