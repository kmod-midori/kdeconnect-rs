use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{IncomingPacket, KdeConnectPlugin, KdeConnectPluginMetadata};

use windows::Win32::UI::Input::KeyboardAndMouse;

const PACKET_TYPE_MOUSEPAD_REQUEST: &str = "kdeconnect.mousepad.request";

#[derive(Debug)]
pub struct ReceiveMousePlugin;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
enum MouseDelta {
    Int(i32),
    Float(f32),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MousePadRequestPacket {
    #[serde(default)]
    singleclick: bool,
    #[serde(default)]
    doubleclick: bool,
    #[serde(default)]
    middleclick: bool,
    #[serde(default)]
    rightclick: bool,
    #[serde(default)]
    singlehold: bool,
    #[serde(default)]
    scroll: bool,

    #[serde(default)]
    alt: bool,
    #[serde(default)]
    ctrl: bool,
    #[serde(default)]
    shift: bool,
    #[serde(default, rename = "super")]
    xuper: bool,

    dx: Option<MouseDelta>,
    dy: Option<MouseDelta>,

    special_key: Option<u32>,
    key: Option<String>,
}

impl ReceiveMousePlugin {}

#[async_trait::async_trait]
impl KdeConnectPlugin for ReceiveMousePlugin {
    async fn handle(&self, packet: IncomingPacket) -> Result<()> {
        match packet.inner.typ.as_str() {
            PACKET_TYPE_MOUSEPAD_REQUEST => {
                let request: MousePadRequestPacket = packet.inner.into_body()?;

                let mut inputs = vec![];

                if let (Some(MouseDelta::Int(dx)), Some(MouseDelta::Int(dy)), false) =
                    (request.dx, request.dy, request.scroll)
                {
                    // Short path for smooth mouse movement, we should never have other fields set in this case.
                    let mouse_input = KeyboardAndMouse::MOUSEINPUT {
                        dx,
                        dy,
                        dwFlags: KeyboardAndMouse::MOUSEEVENTF_MOVE,
                        ..Default::default()
                    };
                    unsafe {
                        KeyboardAndMouse::SendInput(
                            &[KeyboardAndMouse::INPUT {
                                r#type: KeyboardAndMouse::INPUT_MOUSE,
                                Anonymous: KeyboardAndMouse::INPUT_0 { mi: mouse_input },
                            }],
                            std::mem::size_of::<KeyboardAndMouse::INPUT>() as i32,
                        );
                    }
                    return Ok(());
                }

                log::info!("Mousepad request: {:?}", request);

                let mut mouse_click_down = KeyboardAndMouse::MOUSE_EVENT_FLAGS::default();
                let mut mouse_click_up = KeyboardAndMouse::MOUSE_EVENT_FLAGS::default();
                if request.singleclick {
                    mouse_click_down |= KeyboardAndMouse::MOUSEEVENTF_LEFTDOWN;
                    mouse_click_up |= KeyboardAndMouse::MOUSEEVENTF_LEFTUP;
                }
                if request.rightclick {
                    mouse_click_down |= KeyboardAndMouse::MOUSEEVENTF_RIGHTDOWN;
                    mouse_click_up |= KeyboardAndMouse::MOUSEEVENTF_RIGHTUP;
                }
                if request.middleclick {
                    mouse_click_down |= KeyboardAndMouse::MOUSEEVENTF_MIDDLEDOWN;
                    mouse_click_up |= KeyboardAndMouse::MOUSEEVENTF_MIDDLEUP;
                }
                if mouse_click_down != KeyboardAndMouse::MOUSE_EVENT_FLAGS::default() {
                    let down = KeyboardAndMouse::INPUT {
                        r#type: KeyboardAndMouse::INPUT_MOUSE,
                        Anonymous: KeyboardAndMouse::INPUT_0 {
                            mi: KeyboardAndMouse::MOUSEINPUT {
                                dwFlags: mouse_click_down,
                                ..Default::default()
                            },
                        },
                    };
                    let mut up = down.clone();
                    up.Anonymous.mi.dwFlags = mouse_click_up;
                    inputs.push(down);
                    inputs.push(up);
                }

                if request.doubleclick {
                    let down = KeyboardAndMouse::INPUT {
                        r#type: KeyboardAndMouse::INPUT_MOUSE,
                        Anonymous: KeyboardAndMouse::INPUT_0 {
                            mi: KeyboardAndMouse::MOUSEINPUT {
                                dwFlags: KeyboardAndMouse::MOUSEEVENTF_LEFTDOWN,
                                ..Default::default()
                            },
                        },
                    };
                    
                    let mut up = down.clone();
                    up.Anonymous.mi.dwFlags = KeyboardAndMouse::MOUSEEVENTF_LEFTUP;

                    inputs.push(down.clone());
                    inputs.push(up.clone());
                    inputs.push(down);
                    inputs.push(up);
                }

                if !inputs.is_empty() {
                    unsafe {
                        KeyboardAndMouse::SendInput(
                            inputs.as_slice(),
                            std::mem::size_of::<KeyboardAndMouse::INPUT>() as i32,
                        );
                    }
                }
                // if let (Some(dx), Some(dy), true) = (request.dx, request.dy, request.scroll) {}
            }
            _ => {}
        }
        Ok(())
    }
}

impl KdeConnectPluginMetadata for ReceiveMousePlugin {
    fn incoming_capabilities() -> Vec<String> {
        vec![PACKET_TYPE_MOUSEPAD_REQUEST.into()]
    }
    fn outgoing_capabilities() -> Vec<String> {
        vec![]
    }
}
