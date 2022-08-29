pub mod handle;
pub mod manager;

use anyhow::Result;
use std::net::SocketAddr;
use tokio::sync::{mpsc, oneshot};

pub use handle::DeviceHandle;
pub use manager::{DeviceManagerActor, DeviceManagerHandle};

use crate::{
    event::KdeConnectEvent,
    packet::{NetworkPacket, NetworkPacketWithPayload},
};

use self::manager::ConnectionId;

#[derive(Debug)]
pub enum Message {
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
    UpdateTray,
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
