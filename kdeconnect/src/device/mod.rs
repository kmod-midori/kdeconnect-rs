pub mod handle;
pub mod manager;

use anyhow::Result;
use std::net::IpAddr;
use tokio::sync::{mpsc, oneshot};

pub use handle::DeviceHandle;
pub use manager::{DeviceManagerActor, DeviceManagerHandle};

use crate::{
    event::SystemEvent,
    packet::{NetworkPacket, NetworkPacketWithPayload},
};

use self::manager::ConnectionId;

#[derive(Debug)]
pub enum Message {
    AddDevice {
        id: String,
        name: String,
        ip: IpAddr,
        conn_id: ConnectionId,
        tx: mpsc::Sender<NetworkPacketWithPayload>,
        reply: oneshot::Sender<DeviceHandle>,
    },
    /// Whether the device is connected
    QueryDevice {
        id: String,
        reply: oneshot::Sender<bool>,
    },
    RemoveDevice {
        id: String,
        conn_id: ConnectionId,
    },
    SendPacket {
        device_id: Option<String>,
        packet: NetworkPacketWithPayload,
    },
    Event(SystemEvent),
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
