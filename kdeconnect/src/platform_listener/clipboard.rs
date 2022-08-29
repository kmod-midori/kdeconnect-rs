use anyhow::Result;
use tao::event_loop::{EventLoop, EventLoopProxy};

use windows::{
    core::{HSTRING, PCWSTR},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::{
            DataExchange::{AddClipboardFormatListener, RemoveClipboardFormatListener},
            LibraryLoader::GetModuleHandleW,
        },
        UI::{
            Shell::{DefSubclassProc, SetWindowSubclass},
            WindowsAndMessaging::{
                self, DefWindowProcW, DestroyWindow, IsWindow, RegisterClassW, CW_USEDEFAULT,
                HMENU, WINDOW_STYLE, WM_CLIPBOARDUPDATE, WM_DESTROY, WS_EX_LAYERED,
                WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
            },
        },
    },
};

use crate::CustomWindowEvent;

pub struct ClipboardListener {
    hwnd: HWND,
}

impl ClipboardListener {
    pub fn new(event_loop: &EventLoop<CustomWindowEvent>) -> Result<Self> {
        unsafe {
            let wnd_class_name = HSTRING::from("kde_connect_rs_clipboard");

            let hinstance = GetModuleHandleW(PCWSTR::null())?;

            let wnd_class = WindowsAndMessaging::WNDCLASSW {
                lpfnWndProc: Some(crate::utils::call_default_window_proc),
                hInstance: hinstance,
                lpszClassName: (&wnd_class_name).into(),
                ..Default::default()
            };
            RegisterClassW(&wnd_class);

            let hwnd = WindowsAndMessaging::CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_TOOLWINDOW,
                &wnd_class_name,
                PCWSTR::null(),
                WINDOW_STYLE::default(),
                CW_USEDEFAULT,
                0,
                CW_USEDEFAULT,
                0,
                HWND::default(),
                HMENU::default(),
                hinstance,
                std::ptr::null_mut(),
            );

            if !IsWindow(hwnd).as_bool() {
                anyhow::bail!("CreateWindowExW failed");
            }

            let subclass_data = Box::new(SubclassData {
                proxy: event_loop.create_proxy(),
            });

            SetWindowSubclass(
                hwnd,
                Some(subclass_proc),
                233,
                Box::into_raw(subclass_data) as _,
            );

            AddClipboardFormatListener(hwnd).ok()?;

            Ok(ClipboardListener { hwnd })
        }
    }
}

impl Drop for ClipboardListener {
    fn drop(&mut self) {
        unsafe {
            RemoveClipboardFormatListener(self.hwnd);
            DestroyWindow(self.hwnd);
        }
    }
}

struct SubclassData {
    proxy: EventLoopProxy<CustomWindowEvent>,
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    subclass_data_ptr: usize,
) -> LRESULT {
    let subclass_data_ptr = subclass_data_ptr as *mut SubclassData;
    if msg == WM_DESTROY {
        Box::from_raw(subclass_data_ptr);
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let subclass_data = &mut *(subclass_data_ptr);

    match msg {
        WM_CLIPBOARDUPDATE => {
            subclass_data
                .proxy
                .send_event(CustomWindowEvent::ClipboardUpdated)
                .ok();
        }
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}
