use anyhow::Result;

use windows::{
    core::{HSTRING, PCWSTR},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            self, DefWindowProcW, CW_USEDEFAULT, GWL_USERDATA, HMENU, WINDOW_EX_STYLE, WINDOW_STYLE,
        },
    },
};

use crate::event::{EventSender, KdeConnectEvent};

pub struct MyWindow {
    hwnd: HWND,
    event_tx: EventSender,
}

impl MyWindow {
    pub fn create(event_tx: EventSender) -> Result<()> {
        unsafe {
            let wnd_class_name = HSTRING::from("KDEConnectRustCls");
            let hinstance = GetModuleHandleW(PCWSTR::null())?;
            let wnd_class = WindowsAndMessaging::WNDCLASSW {
                lpfnWndProc: Some(MyWindow::winproc),
                hInstance: hinstance,
                lpszClassName: (&wnd_class_name).into(),
                ..Default::default()
            };
            WindowsAndMessaging::RegisterClassW(&wnd_class);

            let my_window = Box::new(MyWindow {
                hwnd: HWND::default(),
                event_tx,
            });
            let my_window_ptr = Box::into_raw(my_window);

            let hwnd = WindowsAndMessaging::CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                &wnd_class_name,
                &HSTRING::from("KDEConnectRust"),
                WINDOW_STYLE::default(),
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                HWND::default(),
                HMENU::default(),
                GetModuleHandleW(PCWSTR::null())?,
                my_window_ptr as *mut _ as *const _,
            );

            if hwnd == HWND::default() {
                return Err(anyhow::anyhow!("CreateWindowExW failed"));
            }

            if !windows::Win32::System::DataExchange::AddClipboardFormatListener(hwnd).as_bool() {
                return Err(anyhow::anyhow!("AddClipboardFormatListener failed"));
            }

            Ok(())
        }
    }

    fn rust_wndproc(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WindowsAndMessaging::WM_CLIPBOARDUPDATE => {
                self.event_tx
                    .blocking_send(KdeConnectEvent::ClipboardUpdated)
                    .ok();
            }
            WindowsAndMessaging::WM_HOTKEY => {
                log::info!("WM_HOTKEY");
            }
            _ => {
                return unsafe { DefWindowProcW(self.hwnd, msg, wparam, lparam) };
            }
        }

        LRESULT(0)
    }

    pub unsafe extern "system" fn winproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WindowsAndMessaging::WM_CREATE => {
                let create_struct: &mut WindowsAndMessaging::CREATESTRUCTW =
                    &mut *(lparam.0 as *mut _);

                let my_window: &mut MyWindow = &mut *(create_struct.lpCreateParams as *mut _);
                my_window.hwnd = hwnd;
                WindowsAndMessaging::SetWindowLongPtrW(
                    hwnd,
                    GWL_USERDATA,
                    my_window as *mut _ as _,
                );

                my_window.rust_wndproc(msg, wparam, lparam)
            }
            WindowsAndMessaging::WM_NCDESTROY => {
                let window_ptr = WindowsAndMessaging::SetWindowLongPtrW(hwnd, GWL_USERDATA, 0);
                if window_ptr != 0 {
                    let ptr = window_ptr as *mut MyWindow;
                    let mut my_window = Box::from_raw(ptr);
                    my_window.rust_wndproc(msg, wparam, lparam)
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            _ => {
                let window_ptr = WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWL_USERDATA);
                if window_ptr != 0 {
                    let my_window: &mut MyWindow = &mut *(window_ptr as *mut MyWindow);
                    my_window.rust_wndproc(msg, wparam, lparam)
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
        }
    }
}
