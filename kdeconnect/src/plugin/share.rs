use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::{
    device::DeviceHandle,
    packet::NetworkPacket,
    utils::{self, clipboard::ClipboardContent},
};

use super::{KdeConnectPlugin, KdeConnectPluginMetadata};

const PACKET_TYPE_SHARE_REQUEST: &str = "kdeconnect.share.request";
const PACKET_TYPE_SHARE_REQUEST_UPDATE: &str = "kdeconnect.share.request.update";

enum WindowsApiRequest {
    OpenItem(String),
}

fn create_windows_api_thread() -> mpsc::Sender<WindowsApiRequest> {
    use windows::Win32::System::Com::{
        CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };
    use windows::{
        core::{HSTRING, PCWSTR},
        Win32::{
            Foundation::HWND,
            UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        },
    };

    let (sender, mut receiver) = mpsc::channel(1);

    std::thread::spawn(move || {
        unsafe {
            let init_res = CoInitializeEx(
                std::ptr::null(),
                COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE,
            );
            if let Err(e) = init_res {
                log::error!("Failed to initialize COM: {}", e);
            }
        }

        let hs_open = HSTRING::from("open");

        while let Some(req) = receiver.blocking_recv() {
            use WindowsApiRequest::*;

            match req {
                OpenItem(item) => {
                    let ret = unsafe {
                        ShellExecuteW(
                            HWND::default(),
                            &hs_open,
                            &HSTRING::from(item),
                            PCWSTR::null(),
                            PCWSTR::null(),
                            SW_SHOWNORMAL.0 as i32,
                        )
                    };
                    // If the function succeeds, it returns a value greater than 32.
                    // If the function fails, it returns an error value that indicates the cause of the failure.
                    // The return value is cast as an HINSTANCE for backward compatibility with 16-bit Windows applications.
                    if ret.0 <= 32 {
                        let err = windows::core::Error::from_win32();
                        log::error!("Failed to open item: {}", err);
                    }
                }
            }
        }
    });

    sender
}

lazy_static::lazy_static! {
    static ref WINDOWS_API_SENDER: mpsc::Sender<WindowsApiRequest> = {
        create_windows_api_thread()
    };
}

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
                        WINDOWS_API_SENDER
                            .send(WindowsApiRequest::OpenItem(url))
                            .await
                            .ok();
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
