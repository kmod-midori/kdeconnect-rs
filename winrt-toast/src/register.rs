use std::{ffi::OsStr, os::windows::prelude::*, path::Path, ptr::null_mut};

use windows::{
    core::{HSTRING, PCWSTR},
    Win32::{
        Foundation::CloseHandle,
        Storage::FileSystem::{CommitTransaction, CreateTransaction},
        System::Registry::{
            RegCreateKeyTransactedW, RegDeleteValueW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
            KEY_ALL_ACCESS, REG_OPTION_NON_VOLATILE, REG_SZ,
        },
    },
};

use crate::WinToastError;

/// Register the application to Windows registry.
///
/// `icon_path` should be an absolute path to the icon file, otherwise [`WinToastError::InvalidPath`] will be returned.
///
/// For more information on AUMID and registration, see this
/// [Windows documentation](https://docs.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast-desktop-cpp-wrl#step-5-register-with-notification-platform).
pub fn register(aum_id: &str, display_name: &str, icon_path: Option<&Path>) -> crate::Result<()> {
    let registry_path = HSTRING::from(format!("SOFTWARE\\Classes\\AppUserModelId\\{}", aum_id));
    let display_name = to_utf16(display_name);
    let icon_path = if let Some(path) = icon_path {
        if !path.is_absolute() {
            return Err(WinToastError::InvalidPath);
        }
        Some(to_utf16(path))
    } else {
        None
    };

    unsafe {
        let transaction = CreateTransaction(null_mut(), null_mut(), 0, 0, 0, 0, PCWSTR::null())?;
        assert!(!transaction.is_invalid());

        scopeguard::defer! {
            CloseHandle(transaction);
        }

        let mut new_hkey = HKEY::default();
        RegCreateKeyTransactedW(
            HKEY_CURRENT_USER,
            &registry_path,
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_ALL_ACCESS,
            null_mut(),
            &mut new_hkey,
            null_mut(),
            transaction,
            null_mut(),
        )
        .ok()?;
        assert!(!new_hkey.is_invalid());

        RegSetValueExW(
            new_hkey,
            &HSTRING::from("DisplayName"),
            0,
            REG_SZ,
            display_name.as_ptr() as *const _,
            utf16_bytes_len(&display_name) as u32,
        )
        .ok()?;

        let icon_uri_name = HSTRING::from("IconUri");
        if let Some(icon_path) = icon_path {
            RegSetValueExW(
                new_hkey,
                &icon_uri_name,
                0,
                REG_SZ,
                icon_path.as_ptr() as *const _,
                utf16_bytes_len(&icon_path) as u32,
            )
            .ok()?;
        } else {
            RegDeleteValueW(new_hkey, &icon_uri_name).ok()?
        }

        CommitTransaction(transaction).ok()?;
    }

    Ok(())
}

/// Convert to null-terminated UTF-16 bytes
fn to_utf16<P: AsRef<OsStr>>(s: P) -> Vec<u16> {
    s.as_ref()
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect()
}

/// Length of UTF-16 slices in terms of bytes
fn utf16_bytes_len(s: &[u16]) -> usize {
    s.len() * std::mem::size_of::<u16>()
}
