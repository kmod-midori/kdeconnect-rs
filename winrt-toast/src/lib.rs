//! A mostly usable binding to the Windows `ToastNotification` API.
//! 
//! # Example
//! ```no_run
//! use winrt_toast::{Toast, Text, Header, ToastManager};
//! use winrt_toast::content::text::TextPlacement;
//!
//! let manager = ToastManager::new("YourCompany.YourApp");
//!
//! let mut toast = Toast::new();
//! toast
//!     .text1("Title")
//!     .text2(Text::new("Body"))
//!     .text3(
//!         Text::new("Via SMS")
//!             .with_placement(TextPlacement::Attribution)
//!     );
//!
//! manager.show(&toast).expect("Failed to show toast");
//!
//! // Or you may add callbacks
//! manager.show_with_callbacks(
//!     &toast, None, None,
//!     Some(Box::new(move |e| {
//!         // This will be called if Windows fails to show the toast.
//!         eprintln!("Failed to show toast: {:?}", e);
//!     }))
//! ).expect("Failed to show toast");
//! ```

#![warn(missing_docs)]

/// Contents in a toast notification.
pub mod content;
pub use content::header::Header;
pub use content::image::Image;
pub use content::text::Text;

mod manager;
pub use manager::{DismissalReason, ToastManager};

mod toast;
pub use toast::Toast;

mod register;
pub use register::register;

/// Re-export of the `url` crate.
pub use url;
use windows::core::HSTRING;

/// Convert a string to a HSTRING
pub(crate) fn hs(s: impl AsRef<str>) -> HSTRING {
    let s = s.as_ref();
    HSTRING::from(s)
}

/// The error type used in this crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WinToastError {
    /// External error from the Windows API.
    #[error("Windows API error: {0}")]
    Os(#[from] windows::core::Error),
    /// Error from the Windows Runtime.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// The given path is not absolute, and therefore cannot be converted to an URL.
    #[error("The given path is not absolute")]
    InvalidPath,
    /// The dismissal reason from OS is unknown
    #[error("The dismissal reason from OS is unknown")]
    InvalidDismissalReason,
}

/// The result type used in this crate.
pub type Result<T> = std::result::Result<T, WinToastError>;
