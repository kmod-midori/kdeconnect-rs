use std::{fmt::Debug, sync::Arc};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::{config::Config, utils};

pub const PACKET_TYPE_IDENTITY: &str = "kdeconnect.identity";
pub const PACKET_TYPE_PAIR: &str = "kdeconnect.pair";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", content = "body")]
pub enum PacketType {
    #[serde(rename = "kdeconnect.battery.request", rename_all = "camelCase")]
    BatteryRequest { request: bool },
    #[serde(rename = "kdeconnect.clipboard", rename_all = "camelCase")]
    Clipboard { content: Option<String> },
    #[serde(rename = "kdeconnect.clipboard.connect", rename_all = "camelCase")]
    ClipboardConnect {
        timestamp: u64,
        content: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairPacket {
    pair: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityPacket {
    pub device_id: String,
    pub device_name: String,
    pub protocol_version: u8,
    pub device_type: String,
    pub incoming_capabilities: Vec<String>,
    pub outgoing_capabilities: Vec<String>,
    pub tcp_port: Option<u16>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPacket {
    // #[serde(flatten)]
    // pub body: PacketType,
    #[serde(rename = "type")]
    pub typ: String,
    pub body: Value,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_transfer_info: Option<PayloadTransferInfo>,
}

impl NetworkPacket {
    pub fn new<B>(typ: impl Into<String>, body: B) -> Self
    where
        B: Serialize,
    {
        Self {
            typ: typ.into(),
            body: serde_json::to_value(body).expect("Failed to serialize body"),
            id: utils::unix_ts_ms(),
            payload_size: None,
            payload_transfer_info: None,
        }
    }

    pub fn new_identity<P, I, O>(tcp_port: P, in_caps: I, out_caps: O, config: &Config) -> Self
    where
        P: Into<Option<u16>>,
        I: IntoIterator<Item = String>,
        O: IntoIterator<Item = String>,
    {
        Self::new(
            PACKET_TYPE_IDENTITY,
            IdentityPacket {
                device_id: config.uuid.clone(),
                device_name: gethostname::gethostname().to_string_lossy().to_string(),
                protocol_version: 7,
                device_type: "desktop".into(),
                incoming_capabilities: in_caps.into_iter().collect(),
                outgoing_capabilities: out_caps.into_iter().collect(),
                tcp_port: tcp_port.into(),
            },
        )
    }

    pub fn new_pair(pair: bool) -> Self {
        Self::new(PACKET_TYPE_PAIR, PairPacket { pair })
    }

    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("Failed to serialize packet")
    }

    /// Reset the timestamp of the packet to the current time.
    pub fn reset_ts(&mut self) {
        self.id = utils::unix_ts_ms();
    }

    pub async fn write_to_conn<W: AsyncWrite + Unpin>(
        &self,
        mut conn: W,
    ) -> Result<(), std::io::Error> {
        conn.write_all(&self.to_vec()).await?;
        conn.write_all(b"\n").await?;
        conn.flush().await?;
        Ok(())
    }

    pub fn into_body<B>(self) -> Result<B, serde_json::Error>
    where
        B: DeserializeOwned,
    {
        serde_json::from_value(self.body)
    }

    pub fn set_payload(&mut self, size: u64, port: u16) {
        self.payload_size = Some(size);
        self.payload_transfer_info = Some(PayloadTransferInfo { port });
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PayloadTransferInfo {
    pub port: u16,
}

#[derive(Clone)]
pub struct NetworkPacketWithPayload {
    pub packet: NetworkPacket,
    pub payload: Option<Arc<Vec<u8>>>,
}

impl Debug for NetworkPacketWithPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let payload_desc = match &self.payload {
            Some(p) => format!("Some({} bytes)", p.len()),
            None => "None".to_string(),
        };

        f.debug_struct("NetworkPacketWithPayload")
            .field("packet", &self.packet)
            .field("payload", &payload_desc)
            .finish()
    }
}

impl From<NetworkPacket> for NetworkPacketWithPayload {
    fn from(packet: NetworkPacket) -> Self {
        Self {
            packet,
            payload: None,
        }
    }
}

impl NetworkPacketWithPayload {
    pub fn new(packet: NetworkPacket, payload: Arc<Vec<u8>>) -> Self {
        Self {
            packet,
            payload: Some(payload),
        }
    }
}
