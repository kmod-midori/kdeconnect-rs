use anyhow::Result;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tao::menu::{ContextMenu, MenuItem, MenuItemAttributes};

use tokio::{
    io::AsyncReadExt,
    sync::{mpsc, oneshot},
};

use crate::{
    context::AppContextRef, device::DeviceHandle, event::KdeConnectEvent,
    packet::NetworkPacketWithPayload, plugin::PluginRepository, utils, CustomWindowEvent,
};

use super::Message;

static NEXT_CONN_ID: AtomicUsize = AtomicUsize::new(0);

fn load_png_icon(buf: &[u8]) -> tao::system_tray::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(buf).unwrap().into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tao::system_tray::Icon::from_rgba(icon_rgba, icon_width, icon_height).unwrap()
}

lazy_static::lazy_static! {
    static ref ICON_CELLPHONE: tao::system_tray::Icon = {
        load_png_icon(include_bytes!("../icons/cellphone.png"))
    };
    static ref ICON_CELLPHONE_OFF: tao::system_tray::Icon = {
        load_png_icon(include_bytes!("../icons/cellphone-off.png"))
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionId(usize);

#[derive(Debug, Clone)]
pub struct DeviceManagerHandle {
    sender: mpsc::Sender<Message>,
    active_device_count: Arc<AtomicUsize>,
}

impl DeviceManagerHandle {
    pub async fn add_device(
        &self,
        id: impl Into<String>,
        name: impl Into<String>,
        addr: SocketAddr,
    ) -> Result<(
        ConnectionId,
        mpsc::Receiver<NetworkPacketWithPayload>,
        DeviceHandle,
    )> {
        let (tx, rx) = mpsc::channel(1);
        let conn_id = ConnectionId(NEXT_CONN_ID.fetch_add(1, Ordering::Relaxed));

        let (reply_tx, reply_rx) = oneshot::channel();

        let msg = Message::AddDevice {
            id: id.into(),
            name: name.into(),
            addr,
            conn_id,
            tx,
            reply: reply_tx,
        };
        self.send_message(msg).await;

        Ok((
            conn_id,
            rx,
            reply_rx
                .await
                .map_err(|_| anyhow::anyhow!("Failed to get device handle"))?,
        ))
    }

    pub async fn remove_device(&self, id: impl Into<String>, conn_id: ConnectionId) {
        let msg = Message::RemoveDevice {
            id: id.into(),
            conn_id,
        };
        self.send_message(msg).await;
    }

    pub(super) async fn send_message(&self, msg: Message) {
        self.sender.send(msg).await.expect("Failed to send message");
    }

    pub fn active_device_count(&self) -> usize {
        self.active_device_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Broadcast an event to all plugins.
    pub async fn broadcast_event(&self, event: KdeConnectEvent) {
        self.send_message(Message::Event(event)).await;
    }

    pub async fn update_tray(&self) {
        self.send_message(Message::UpdateTray).await;
    }

    pub async fn send_packet(&self, device_id: &str, packet: impl Into<NetworkPacketWithPayload>) {
        let packet: NetworkPacketWithPayload = packet.into();

        let msg = Message::SendPacket {
            device_id: Some(device_id.into()),
            packet,
        };
        self.send_message(msg).await;
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct Device {
    name: String,
    remote_addr: SocketAddr,
    conn_id: ConnectionId,
    tx: mpsc::Sender<NetworkPacketWithPayload>,
    plugin_repo: Arc<PluginRepository>,
}

pub struct DeviceManagerActor {
    receiver: mpsc::Receiver<Message>,
    devices: HashMap<String, Device>,
    active_device_count: Arc<AtomicUsize>,
    handle: DeviceManagerHandle,
}

impl DeviceManagerActor {
    pub fn new() -> (Self, DeviceManagerHandle) {
        let (sender, receiver) = mpsc::channel(100);
        let active_device_count = Arc::new(AtomicUsize::new(0));

        let handle = DeviceManagerHandle {
            sender,
            active_device_count: active_device_count.clone(),
        };

        let actor = Self {
            receiver,
            devices: HashMap::new(),
            active_device_count,
            handle: handle.clone(),
        };

        (actor, handle)
    }

    async fn handle_message(&mut self, msg: Message, ctx: &AppContextRef) {
        let mut tray_updated = false;

        match msg {
            Message::AddDevice {
                id,
                name,
                addr,
                conn_id,
                tx,
                reply,
            } => {
                let dh = DeviceHandle {
                    device_id: Arc::new(id.clone()),
                    device_name: Arc::new(name.clone()),
                    manager_handle: self.handle.clone(),
                };

                log::info!("Adding device: {}", id);
                utils::simple_toast("Device Connected", None, Some(&name)).await;

                if let Some(device) = self.devices.get_mut(&id) {
                    device.remote_addr = addr;
                    device.conn_id = conn_id;
                    device.tx = tx;
                } else {
                    let plugin_repo = PluginRepository::new(dh.clone(), ctx.clone()).await;
                    self.devices.insert(
                        id,
                        Device {
                            name,
                            remote_addr: addr,
                            conn_id,
                            tx,
                            plugin_repo: Arc::new(plugin_repo),
                        },
                    );
                }

                let _ = reply.send(dh);

                self.update_active_device_count();

                tray_updated = true;
            }
            Message::RemoveDevice { id, conn_id } => {
                if let Some(device) = self.devices.get_mut(&id) {
                    if device.conn_id == conn_id {
                        // We are still on the same connection, so we can remove the device
                        log::info!("Removed device: {}", id);
                        utils::simple_toast("Device Disconnected", None, Some(&device.name)).await;

                        self.devices.remove(&id);
                        self.update_active_device_count();
                    }
                }

                tray_updated = true;
            }
            Message::SendPacket { packet, device_id } => {
                if let Some(device_id) = device_id {
                    log::debug!("Sending {:?} to {}", packet, device_id);

                    if let Some(device) = self.devices.get(&device_id) {
                        if let Err(e) = device.tx.send(packet).await {
                            log::error!("Failed to send packet to device {}: {}", device.name, e);
                        }
                    }
                } else {
                    log::debug!("Broadcasting {:?}", packet);

                    for device in self.devices.values() {
                        if let Err(e) = device.tx.send(packet.clone()).await {
                            log::error!("Failed to send packet to device {}: {}", device.name, e);
                        };
                    }
                }
            }
            Message::Event(event) => {
                for device in self.devices.values() {
                    let pr = device.plugin_repo.clone();
                    let event = event.clone();

                    tokio::spawn(async move {
                        pr.handle_event(event).await;
                    });
                }
            }
            Message::Packet { device_id, packet } => {
                let device = if let Some(device) = self.devices.get_mut(&device_id) {
                    device
                } else {
                    log::warn!("Device {} not found", device_id);
                    return;
                };
                let pr = device.plugin_repo.clone();

                tokio::spawn(async move {
                    if let Err(e) = pr.handle_packet(packet).await {
                        log::error!("Failed to handle packet from {}: {:?}", device_id, e);
                    }
                });
            }
            Message::FetchPayload {
                device_id,
                port,
                size,
                reply,
            } => {
                let device = if let Some(device) = self.devices.get_mut(&device_id) {
                    device
                } else {
                    let _ = reply.send(Err(anyhow::anyhow!("Device {} not found", device_id)));
                    return;
                };
                let remote_ip = device.remote_addr.ip();
                let ctx = ctx.clone();

                tokio::spawn(async move {
                    let task = async {
                        let mut conn = ctx.tls_connect((remote_ip, port)).await?;
                        let mut buf = Vec::with_capacity(size as usize);
                        conn.read_to_end(&mut buf).await?;

                        if buf.len() == size {
                            Ok(buf)
                        } else {
                            Err(anyhow::anyhow!(
                                "Payload size mismatch: {} (fetched) != {} (requested)",
                                buf.len(),
                                size
                            ))
                        }
                    };
                    let _ = reply.send(task.await);
                });
            }
            Message::UpdateTray => {
                tray_updated = true;
            }
        }

        if tray_updated {
            self.update_tray(ctx).await;
        }
    }

    fn update_active_device_count(&self) {
        let count = self.devices.len();
        self.active_device_count
            .store(count, std::sync::atomic::Ordering::Relaxed);
    }

    async fn update_tray(&self, ctx: &AppContextRef) {
        let mut menu = ContextMenu::new();

        let mut tasks = vec![];

        if self.devices.is_empty() {
            menu.add_item(MenuItemAttributes::new("No device connected").with_enabled(false));
        } else {
            for device in self.devices.values() {
                let pr = device.plugin_repo.clone();
                let device_name = device.name.clone();
                let task = tokio::spawn(async move { (device_name, pr.create_tray_menu().await) });
                tasks.push(task);
            }

            for (name, submenu) in futures::future::join_all(tasks).await.into_iter().flatten() {
                menu.add_submenu(&name, true, submenu);
            }
        }

        menu.add_native_item(MenuItem::Separator);
        menu.add_native_item(MenuItem::Quit);

        ctx.event_loop_proxy
            .send_event(CustomWindowEvent::SetTrayMenu(menu))
            .ok();

        let icon = if self.devices.is_empty() {
            ICON_CELLPHONE_OFF.clone()
        } else {
            ICON_CELLPHONE.clone()
        };
        ctx.event_loop_proxy
            .send_event(CustomWindowEvent::SetTrayIcon(icon))
            .ok();
    }

    /// Spawn the actor to a background task.
    pub fn run(mut self, ctx: AppContextRef) {
        tokio::spawn(async move {
            self.update_tray(&ctx).await;

            while let Some(msg) = self.receiver.recv().await {
                self.handle_message(msg, &ctx).await;
            }
        });
    }
}
