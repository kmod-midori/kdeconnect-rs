use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use tokio::sync::mpsc;

use crate::packet::NetworkPacket;

static NEXT_CONN_ID: AtomicUsize = AtomicUsize::new(0);

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
    ) -> (ConnectionId, mpsc::Receiver<Vec<u8>>) {
        let (tx, rx) = mpsc::channel(1);
        let conn_id = ConnectionId(NEXT_CONN_ID.fetch_add(1, Ordering::Relaxed));

        let msg = Message::AddDevice {
            id: id.into(),
            name: name.into(),
            addr,
            conn_id,
            tx,
        };
        self.send_message(msg).await;

        (conn_id, rx)
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

    pub fn active_device_count(&self) -> usize {
        self.active_device_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn broadcast_packet(&self, packet: NetworkPacket) {
        log::debug!("Broadcasting {:?}", packet);

        let msg = Message::BroadcastPacket { packet: packet.to_vec() };
        self.send_message(msg).await;
    }
}

#[derive(Debug)]
enum Message {
    AddDevice {
        id: String,
        name: String,
        addr: SocketAddr,
        conn_id: ConnectionId,
        tx: mpsc::Sender<Vec<u8>>,
    },
    RemoveDevice {
        id: String,
        conn_id: ConnectionId,
    },
    BroadcastPacket {
        packet: Vec<u8>,
    },
}

#[derive(Debug)]
struct Device {
    name: String,
    _remote_addr: SocketAddr,
    conn_id: ConnectionId,
    tx: mpsc::Sender<Vec<u8>>,
}

pub struct DeviceManagerActor {
    receiver: mpsc::Receiver<Message>,
    devices: HashMap<String, Device>,
    active_device_count: Arc<AtomicUsize>,
}

impl DeviceManagerActor {
    pub fn new() -> (Self, DeviceManagerHandle) {
        let (sender, receiver) = mpsc::channel(100);
        let actor = Self {
            receiver,
            devices: HashMap::new(),
            active_device_count: Arc::new(AtomicUsize::new(0)),
        };

        let handle = DeviceManagerHandle {
            sender,
            active_device_count: actor.active_device_count.clone(),
        };

        (actor, handle)
    }

    async fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::AddDevice {
                id,
                name,
                addr,
                conn_id,
                tx,
            } => {
                log::info!("Added device: {}", id);
                self.devices.insert(
                    id,
                    Device {
                        name,
                        _remote_addr: addr,
                        conn_id,
                        tx,
                    },
                );

                self.update_active_device_count();
            }
            Message::RemoveDevice { id, conn_id } => {
                if let Some(device) = self.devices.get_mut(&id) {
                    if device.conn_id == conn_id {
                        // We are still on the same connection, so we can remove the device
                        log::info!("Removed device: {}", id);
                        self.devices.remove(&id);
                        self.update_active_device_count();
                    }
                }
            }
            Message::BroadcastPacket { packet } => {
                for device in self.devices.values() {
                    if let Err(e) = device.tx.send(packet.clone()).await {
                        log::error!("Failed to send packet to device {}: {}", device.name, e);
                    };
                }
            }
        }
    }

    fn update_active_device_count(&self) {
        let count = self.devices.len();
        self.active_device_count
            .store(count, std::sync::atomic::Ordering::Relaxed);
    }

    /// Spawn the actor to a background task.
    pub fn run(mut self) {
        tokio::spawn(async move {
            while let Some(msg) = self.receiver.recv().await {
                self.handle_message(msg).await;
            }
        });
    }
}
