use anyhow::Result;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use tokio::{
    io::AsyncReadExt,
    sync::{mpsc, oneshot},
};

use crate::{
    context::AppContextRef,
    event::KdeConnectEvent,
    packet::{NetworkPacket, NetworkPacketWithPayload},
    plugin::PluginRepository,
    utils,
};

static NEXT_CONN_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionId(usize);

#[derive(Clone)]
pub struct DeviceHandle {
    device_id: Arc<String>,
    device_name: Arc<String>,
    manager_handle: DeviceManagerHandle,
}

impl std::fmt::Debug for DeviceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceHandle")
            .field("device_id", &self.device_id)
            .finish()
    }
}

impl DeviceHandle {
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Send packet to device
    pub async fn send_packet(&self, packet: impl Into<NetworkPacketWithPayload>) {
        self.manager_handle
            .send_packet(self.device_id(), packet)
            .await;
    }

    pub fn blocking_send_packet(&self, packet: impl Into<NetworkPacketWithPayload>) {
        self.manager_handle
            .blocking_send_packet(self.device_id(), packet);
    }

    /// Dispatch received packet from the device to plugins
    pub async fn dispatch_packet(&self, packet: impl Into<NetworkPacket>) {
        self.manager_handle
            .send_message(Message::Packet {
                device_id: self.device_id.to_string(),
                packet: packet.into(),
            })
            .await;
    }

    pub async fn fetch_payload(&self, port: u16, size: usize) -> Result<Vec<u8>> {
        let (tx, rx) = oneshot::channel();

        self.manager_handle
            .send_message(Message::FetchPayload {
                device_id: self.device_id.to_string(),
                port,
                size,
                reply: tx,
            })
            .await;

        rx.await?
    }
}

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

    async fn send_message(&self, msg: Message) {
        self.sender.send(msg).await.expect("Failed to send message");
    }

    fn blocking_send_message(&self, msg: Message) {
        self.sender
            .blocking_send(msg)
            .expect("Failed to send message");
    }

    pub fn active_device_count(&self) -> usize {
        self.active_device_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Broadcast an event to all plugins.
    pub async fn broadcast_event(&self, event: KdeConnectEvent) {
        self.send_message(Message::Event(event)).await;
    }

    pub async fn send_packet(&self, device_id: &str, packet: impl Into<NetworkPacketWithPayload>) {
        let packet: NetworkPacketWithPayload = packet.into();

        let msg = Message::SendPacket {
            device_id: Some(device_id.into()),
            packet,
        };
        self.send_message(msg).await;
    }

    pub fn blocking_send_packet(
        &self,
        device_id: &str,
        packet: impl Into<NetworkPacketWithPayload>,
    ) {
        let packet: NetworkPacketWithPayload = packet.into();

        let msg = Message::SendPacket {
            device_id: Some(device_id.into()),
            packet,
        };
        self.blocking_send_message(msg);
    }
}

#[derive(Debug)]
enum Message {
    AddDevice {
        id: String,
        name: String,
        addr: SocketAddr,
        conn_id: ConnectionId,
        tx: mpsc::Sender<NetworkPacketWithPayload>,
        reply: oneshot::Sender<DeviceHandle>,
    },
    RemoveDevice {
        id: String,
        conn_id: ConnectionId,
    },
    SendPacket {
        device_id: Option<String>,
        packet: NetworkPacketWithPayload,
    },
    Event(KdeConnectEvent),
    Packet {
        device_id: String,
        packet: NetworkPacket,
    },
    FetchPayload {
        device_id: String,
        port: u16,
        size: usize,
        reply: oneshot::Sender<Result<Vec<u8>>>,
    },
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
        }
    }

    fn update_active_device_count(&self) {
        let count = self.devices.len();
        self.active_device_count
            .store(count, std::sync::atomic::Ordering::Relaxed);
    }

    /// Spawn the actor to a background task.
    pub fn run(mut self, ctx: AppContextRef) {
        tokio::spawn(async move {
            while let Some(msg) = self.receiver.recv().await {
                self.handle_message(msg, &ctx).await;
            }
        });
    }
}
