use anyhow::Result;
use tokio::sync::{mpsc, oneshot};

enum RequestType {
    OpenItem(String),
}

struct WindowsApiRequest {
    ty: RequestType,
    response: oneshot::Sender<Result<()>>,
}

impl WindowsApiRequest {
    fn new(ty: RequestType) -> (Self, oneshot::Receiver<Result<()>>) {
        let (tx, rx) = oneshot::channel();
        (Self { ty, response: tx }, rx)
    }
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

    let (sender, mut receiver) = mpsc::channel::<WindowsApiRequest>(1);

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
            use RequestType::*;

            let res = match req.ty {
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
                    if ret.0 > 32 {
                        Ok(())
                    } else {
                        Err(windows::core::Error::from_win32().into())
                    }
                }
            };

            let _ = req.response.send(res);
        }
    });

    sender
}

lazy_static::lazy_static! {
    static ref WINDOWS_API_SENDER: mpsc::Sender<WindowsApiRequest> = {
        create_windows_api_thread()
    };
}

pub async fn open_url(url: impl Into<String>) -> Result<()> {
    let (req, rx) = WindowsApiRequest::new(RequestType::OpenItem(url.into()));
    match WINDOWS_API_SENDER.send(req).await {
        Ok(_) => rx.await?,
        Err(_) => Err(anyhow::anyhow!(
            "Failed to send request to Windows API thread (channel closed)"
        )),
    }
}
