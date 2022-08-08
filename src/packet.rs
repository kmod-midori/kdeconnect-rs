use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncWrite, AsyncWriteExt};

fn unix_ts_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", content = "body")]
pub enum PacketType {
    #[serde(rename = "kdeconnect.identity", rename_all = "camelCase")]
    Identity {
        device_id: String,
        device_name: String,
        protocol_version: u8,
        device_type: String,
        incoming_capabilities: Vec<String>,
        outgoing_capabilities: Vec<String>,
        tcp_port: Option<u16>,
    },
    #[serde(rename = "kdeconnect.pair", rename_all = "camelCase")]
    Pair { pair: bool },
    #[serde(rename = "kdeconnect.ping", rename_all = "camelCase")]
    Ping {},
    #[serde(rename = "kdeconnect.notification", rename_all = "camelCase")]
    Notification(Value),
    #[serde(rename = "kdeconnect.battery", rename_all = "camelCase")]
    Battery {
        /// Battery level in percent
        current_charge: u8,
        is_charging: bool,
        /// 1 if battery is low, 0 if not.
        threshold_event: u8, 
    },
    #[serde(rename = "kdeconnect.battery.request", rename_all = "camelCase")]
    BatteryRequest {
        request: bool
    },
    #[serde(rename = "kdeconnect.clipboard", rename_all = "camelCase")]
    Clipboard {
        content: Option<String>
    },
    #[serde(rename = "kdeconnect.clipboard.connect", rename_all = "camelCase")]
    ClipboardConnect {
        timestamp: u64,
        content: Option<String>,
    },
    #[serde(rename = "kdeconnect.connectivity_report", rename_all = "camelCase")]
    ConnectivityReport(Value),
    #[serde(rename = "kdeconnect.connectivity_report.request", rename_all = "camelCase")]
    ConnectivityReportRequest {
        request: bool,
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NetworkPacket {
    #[serde(flatten)]
    pub body: PacketType,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_transfer_info: Option<PayloadTransferInfo>,
}

impl NetworkPacket {
    pub fn new(body: PacketType) -> Self {
        Self {
            body,
            id: unix_ts_ms(),
            payload_size: None,
            payload_transfer_info: None,
        }
    }

    pub fn new_identity(tcp_port: impl Into<Option<u16>>) -> Self {
        Self::new(PacketType::Identity {
            device_id: "_LIVE_BEEF_".into(),
            device_name: "LycoReco".into(),
            protocol_version: 7,
            device_type: "desktop".into(),
            incoming_capabilities: vec![
                "kdeconnect.ping".into(),
                "kdeconnect.notification".into(),
                "kdeconnect.battery".into(),
                "kdeconnect.clipboard".into(),
                "kdeconnect.clipboard.connect".into(),
                "kdeconnect.connectivity_report".into()
            ],
            outgoing_capabilities: vec![
                "kdeconnect.ping".into(),
                "kdeconnect.notification.request".into(),
                "kdeconnect.notification.reply".into(),
                "kdeconnect.clipboard".into(),
                "kdeconnect.clipboard.connect".into(),
                "kdeconnect.connectivity_report.request".into()
            ],
            tcp_port: tcp_port.into(),
        })
    }

    pub fn new_pair(pair: bool) -> Self {
        Self::new(PacketType::Pair { pair })
    }

    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("Failed to serialize packet")
    }

    pub async fn write_to_conn<W: AsyncWrite + Unpin>(
        &self,
        mut conn: W,
    ) -> Result<(), std::io::Error> {
        conn.write_all(&self.to_vec()).await?;
        conn.write(b"\n").await?;
        conn.flush().await?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PayloadTransferInfo {
    pub port: u16,
}
