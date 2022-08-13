//! A mostly usable binding to the Windows `ToastNotification` API.
//! 
//! # Example
//! ```rust
//! # use winrt_toast::{Toast, Text, Header};
//! # use winrt_toast::content::text::TextPlacement;
//!
//! let mut toast = Toast::new()
//!     .text1("Title")
//!     .text2(Text::new("Body"))
//!     .text3(
//!         Text::new("Via SMS")
//!             .with_placement(TextPlacement::Attribution)
//!     );
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
