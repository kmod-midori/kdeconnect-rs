use std::{collections::HashMap, sync::Arc};

use crate::{
    context::AppContextRef,
    device::DeviceHandle,
    event::SystemEvent,
    packet::NetworkPacket,
    plugin::{KdeConnectPlugin, KdeConnectPluginMetadata},
};
use anyhow::Result;
use tao::menu::{ContextMenu, MenuId, MenuItem, MenuItemAttributes};
use tokio::sync::RwLock;

use super::{
    MprisMetadata, MprisPacket, MprisRequest, PACKET_TYPE_MPRIS, PACKET_TYPE_MPRIS_REQUEST,
};

#[derive(Debug)]
struct Player {
    metadata: Option<MprisMetadata>,
    play_menu_id: MenuId,
    previous_menu_id: MenuId,
    next_menu_id: MenuId,
}

impl Player {
    fn new(device_id: &str, player_id: &str) -> Self {
        let prefix = format!("{}:mpris_remote:{}", device_id, player_id);

        Self {
            metadata: None,
            play_menu_id: MenuId::new(&format!("{prefix}:play",)),
            previous_menu_id: MenuId::new(&format!("{prefix}:previous",)),
            next_menu_id: MenuId::new(&format!("{prefix}:next",)),
        }
    }
}

#[derive(Debug)]
pub struct MprisRemotePlugin {
    ctx: AppContextRef,
    dev: DeviceHandle,
    players: RwLock<HashMap<String, Player>>,
}

impl MprisRemotePlugin {
    pub fn new(dev: DeviceHandle, ctx: AppContextRef) -> Self {
        Self {
            ctx,
            dev,
            players: RwLock::new(HashMap::new()),
        }
    }

    async fn request_player_list(&self) {
        self.dev
            .send_packet(NetworkPacket::new(
                PACKET_TYPE_MPRIS_REQUEST,
                MprisRequest {
                    request_now_playing: Some(true),
                    ..Default::default()
                },
            ))
            .await;
    }

    async fn request_now_playing(&self, player_id: &str) {
        self.dev
            .send_packet(NetworkPacket::new(
                PACKET_TYPE_MPRIS_REQUEST,
                MprisRequest {
                    player: Some(player_id.to_string()),
                    request_now_playing: Some(true),
                    ..Default::default()
                },
            ))
            .await;
    }

    async fn send_action(&self, player_id: &str, action: &str) {
        let mut commands = HashMap::new();
        commands.insert("action".to_string(), serde_json::Value::from(action));

        self.dev
            .send_packet(NetworkPacket::new(
                PACKET_TYPE_MPRIS_REQUEST,
                MprisRequest {
                    player: Some(player_id.to_string()),
                    commands,
                    ..Default::default()
                },
            ))
            .await;
    }
}

#[async_trait::async_trait]
impl KdeConnectPlugin for MprisRemotePlugin {
    async fn start(self: Arc<Self>) -> Result<()> {
        self.request_player_list().await;
        Ok(())
    }

    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        let packet = packet.into_body::<MprisPacket>()?;
        match packet {
            MprisPacket::PlayerList { player_list, .. } => {
                {
                    let mut players = self.players.write().await;

                    // Remove players that are no longer present
                    players.retain(|k, _| player_list.contains(k));

                    // Add new players
                    for player in player_list {
                        if players.contains_key(&player) {
                            continue;
                        }
                        players.insert(player.clone(), Player::new(self.dev.device_id(), &player));
                    }
                }
                self.ctx.update_tray().await;
            }
            MprisPacket::Metadata(metadata) => {
                let mut players = self.players.write().await;
                if let Some(player) = players.get_mut(&metadata.properties.player) {
                    player.metadata = Some(metadata);
                    self.ctx.update_tray().await;
                }
            }
            MprisPacket::TransferringAlbumArt { .. } => {
                // Ignore
            }
        }
        Ok(())
    }

    async fn tray_menu(&self, menu: &mut ContextMenu) {
        let players = self.players.read().await;
        if players.is_empty() {
            // Hide the menu
            return;
        }

        let mut submenu = ContextMenu::new();

        for (id, player) in players.iter() {
            if let Some(metadata) = player.metadata.as_ref() {
                let title = format!(
                    "{}\t\t\t  {}",
                    id,
                    if metadata.status.is_playing {
                        "Playing"
                    } else {
                        "Paused"
                    }
                );
                submenu.add_item(MenuItemAttributes::new(&title).with_id(player.play_menu_id));

                if !metadata.properties.now_playing.is_empty() {
                    submenu.add_item(
                        MenuItemAttributes::new(&metadata.properties.now_playing)
                            .with_enabled(false),
                    );
                }
                if metadata.status.can_go_previous {
                    submenu.add_item(
                        MenuItemAttributes::new("Previous").with_id(player.previous_menu_id),
                    );
                }
                if metadata.status.can_go_next {
                    submenu.add_item(MenuItemAttributes::new("Next").with_id(player.next_menu_id));
                }
            } else {
                submenu.add_item(MenuItemAttributes::new(&format!("{}\t\t\t  Unknown", id,)));
            }

            submenu.add_native_item(MenuItem::Separator);
        }

        menu.add_submenu("Media Control", true, submenu)
    }

    async fn handle_event(self: Arc<Self>, event: SystemEvent) -> Result<()> {
        if let SystemEvent::TrayMenuClicked(menu_id) = event {
            let players = self.players.read().await;

            for (id, player) in players.iter() {
                if menu_id == player.play_menu_id {
                    self.send_action(id, "PlayPause").await;
                } else if menu_id == player.previous_menu_id {
                    self.send_action(id, "Previous").await;
                } else if menu_id == player.next_menu_id {
                    self.send_action(id, "Next").await;
                }
            }
        }
        Ok(())
    }
}

impl KdeConnectPluginMetadata for MprisRemotePlugin {
    fn incoming_capabilities() -> Vec<String> {
        vec![PACKET_TYPE_MPRIS.into()]
    }

    fn outgoing_capabilities() -> Vec<String> {
        vec![PACKET_TYPE_MPRIS_REQUEST.into()]
    }
}
