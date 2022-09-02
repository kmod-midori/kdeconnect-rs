/*!
It receives a packages with type kdeconnect.share. If they have a payload
attached, it will download it as a file with the filename set in the field
"filename" (string). If that field is not set it should generate a filename.

If the content transferred is text, it can be sent in a field "text" (string)
instead of an attached payload. In that case, this plugin opens a text editor
with the content instead of saving it as a file.

If the content transferred is a url, it can be sent in a field "url" (string).
In that case, this plugin opens that url in the default browser.
 */
use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    device::DeviceHandle,
    packet::NetworkPacket,
    utils::{self, clipboard::ClipboardContent},
};

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

const PACKET_TYPE_SHARE_REQUEST: &str = "kdeconnect.share.request";
const PACKET_TYPE_SHARE_REQUEST_UPDATE: &str = "kdeconnect.share.request.update";

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum ShareRequestPacket {
    Text { text: String },
    Url { url: String },
}

#[derive(Debug)]
pub struct SharePlugin {
    dev: DeviceHandle,
}

impl SharePlugin {
    pub fn new(dev: DeviceHandle) -> Self {
        SharePlugin {
            dev,
            // ctx,
        }
    }
}

#[async_trait::async_trait]
impl KdeConnectPlugin for SharePlugin {
    async fn handle(&self, packet: NetworkPacket) -> Result<()> {
        match packet.typ.as_str() {
            PACKET_TYPE_SHARE_REQUEST => {
                let body: ShareRequestPacket = packet.into_body()?;
                match body {
                    ShareRequestPacket::Text { text } => {
                        log::info!("Received text: {}", text);
                        tokio::task::spawn_blocking(move || {
                            utils::clipboard::write(ClipboardContent::Text(text))
                        })
                        .await??;
                    }
                    ShareRequestPacket::Url { url } => {
                        log::info!("Received URL: {}", url);
                        utils::open::open_url(url).await?;
                    }
                }
            }
            PACKET_TYPE_SHARE_REQUEST_UPDATE => {}
            _ => {}
        }

        Ok(())
    }
}

impl KdeConnectPluginMetadata for SharePlugin {
    fn incoming_capabilities() -> Vec<String> {
        vec![
            PACKET_TYPE_SHARE_REQUEST.into(),
            PACKET_TYPE_SHARE_REQUEST_UPDATE.into(),
        ]
    }
    fn outgoing_capabilities() -> Vec<String> {
        vec![
            PACKET_TYPE_SHARE_REQUEST.into(),
            PACKET_TYPE_SHARE_REQUEST_UPDATE.into(),
        ]
    }
}
