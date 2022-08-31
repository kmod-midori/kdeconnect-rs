use anyhow::Result;
use tao::event_loop::{EventLoop, EventLoopProxy};

use windows::{
    core::{HSTRING, PCWSTR},
    Win32::{
        Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM},
        System::{
            DataExchange::{AddClipboardFormatListener, RemoveClipboardFormatListener},
            LibraryLoader::GetModuleHandleW,
            Power::{
                RegisterPowerSettingNotification, UnregisterPowerSettingNotification, HPOWERNOTIFY,
            },
            SystemServices::{GUID_ACDC_POWER_SOURCE, GUID_BATTERY_PERCENTAGE_REMAINING},
        },
        UI::{
            Shell::{DefSubclassProc, SetWindowSubclass},
            WindowsAndMessaging::{
                self, DefWindowProcW, DestroyWindow, IsWindow, RegisterClassW, CW_USEDEFAULT,
                HMENU, WINDOW_STYLE, WM_CLIPBOARDUPDATE, WM_DESTROY, WM_POWERBROADCAST,
                WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
            },
        },
    },
};

use crate::CustomWindowEvent;

/// Clipboard and power status listener on Windows.
pub struct WindowsListener {
    hwnd: HWND,
    handle_acdc: HPOWERNOTIFY,
    handle_battery: HPOWERNOTIFY,
}

impl WindowsListener {
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

            let handle_acdc =
                RegisterPowerSettingNotification(HANDLE(hwnd.0), &GUID_ACDC_POWER_SOURCE, 0)?;
            let handle_battery = RegisterPowerSettingNotification(
                HANDLE(hwnd.0),
                &GUID_BATTERY_PERCENTAGE_REMAINING,
                0,
            )?;

            Ok(WindowsListener {
                hwnd,
                handle_acdc,
                handle_battery,
            })
        }
    }
}

impl Drop for WindowsListener {
    fn drop(&mut self) {
        unsafe {
            RemoveClipboardFormatListener(self.hwnd);
            UnregisterPowerSettingNotification(self.handle_acdc);
            UnregisterPowerSettingNotification(self.handle_battery);
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
        WM_POWERBROADCAST => {
            subclass_data
                .proxy
                .send_event(CustomWindowEvent::PowerStatusUpdated)
                .ok();
        }
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}
