//! Test

#![warn(missing_docs)]
use std::{collections::HashMap, time::Duration};
/// Text elements
pub mod text;
pub use text::Text;

/// Image elements
pub mod image;
pub use image::Image;

/// Header elements
pub mod header;
pub use header::Header;

/// Re-export of the `url` crate.
pub use url;
use windows::{
    core::{IInspectable, Interface, HSTRING},
    Data::Xml::Dom::XmlDocument,
    Foundation::{PropertyValue, TypedEventHandler},
    Globalization::Calendar,
    UI::Notifications::{
        ToastActivatedEventArgs, ToastDismissalReason, ToastDismissedEventArgs,
        ToastFailedEventArgs, ToastNotification, ToastNotificationManager,
    },
};

/// Specifies the reason that a toast notification is no longer being shown
///
/// See <https://docs.microsoft.com/en-us/uwp/api/windows.ui.notifications.toastdismissalreason>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DismissalReason {
    /// The user dismissed the toast notification.
    UserCanceled,
    /// The app explicitly hid the toast notification by calling the ToastNotifier.hide method.
    ApplicationHidden,
    /// The toast notification had been shown for the maximum allowed time and was faded out.
    /// The maximum time to show a toast notification is 7 seconds except in the case of long-duration toasts,
    /// in which case it is 25 seconds.
    TimedOut,
}

impl DismissalReason {
    fn from_winrt(reason: ToastDismissalReason) -> Result<Self> {
        match reason {
            ToastDismissalReason::UserCanceled => Ok(DismissalReason::UserCanceled),
            ToastDismissalReason::ApplicationHidden => Ok(DismissalReason::ApplicationHidden),
            ToastDismissalReason::TimedOut => Ok(DismissalReason::TimedOut),
            _ => Err(WinToastError::InvalidDismissalReason),
        }
    }
}

/// An interface that provides access to the toast notification manager.
///
/// This does not actually hold any Windows resource, but is used to
/// store the application ID that is required to access the toast notification manager.
#[derive(Clone)]
pub struct ToastManager {
    app_id: HSTRING,
}

impl std::fmt::Debug for ToastManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ToastManager({})", self.app_id)
    }
}

impl ToastManager {
    /// Create a new manager with 
    pub fn new(app_id: impl AsRef<str>) -> Self {
        Self {
            app_id: hs(app_id.as_ref()),
        }
    }

    /// Remove all notifications in `group`.
    pub fn remove_group(&self, group: &str) -> Result<()> {
        let history = ToastNotificationManager::History()?;

        history.RemoveGroupWithId(&hs(group), &self.app_id)?;

        Ok(())
    }

    /// Remove a notification in `group` with `tag`.
    pub fn remove_grouped_tag(&self, group: &str, tag: &str) -> Result<()> {
        let history = ToastNotificationManager::History()?;

        history.RemoveGroupedTagWithId(&hs(tag), &hs(group), &self.app_id)?;

        Ok(())
    }

    /// Remove a notification with the specified `tag`.
    pub fn remove(&self, tag: &str) -> Result<()> {
        let history = ToastNotificationManager::History()?;

        history.Remove(&hs(tag))?;

        Ok(())
    }

    /// Clear all toast notifications from this application.
    pub fn clear(&self) -> Result<()> {
        let history = ToastNotificationManager::History()?;

        history.ClearWithId(&self.app_id)?;

        Ok(())
    }

    /// Send a toast to Windows for display.
    pub fn show(
        &self,
        in_toast: &Toast,
        on_activated: Option<Box<dyn FnMut(Result<String>) + Send + 'static>>,
        on_dismissed: Option<Box<dyn FnMut(Result<DismissalReason>) + Send + 'static>>,
        on_failed: Option<Box<dyn FnMut(WinToastError) + Send + 'static>>,
    ) -> Result<()> {
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&self.app_id)?;

        let toast_doc = XmlDocument::new()?;

        let toast_el = toast_doc.CreateElement(&hs("toast"))?;
        toast_doc.AppendChild(&toast_el)?;

        // <header>
        if let Some(header) = &in_toast.header {
            let el = toast_doc.CreateElement(&hs("header"))?;
            toast_el.AppendChild(&el)?;
            header.write_to_element(&el)?;
        }
        // </header>
        // <visual>
        {
            let visual_el = toast_doc.CreateElement(&hs("visual"))?;
            toast_el.AppendChild(&visual_el)?;
            // <binding>
            {
                let binding_el = toast_doc.CreateElement(&hs("binding"))?;
                visual_el.AppendChild(&binding_el)?;
                binding_el.SetAttribute(&hs("template"), &hs("ToastGeneric"))?;
                {
                    if let Some(text) = &in_toast.text.0 {
                        let el = toast_doc.CreateElement(&hs("text"))?;
                        binding_el.AppendChild(&el)?;
                        text.write_to_element(1, &el)?;
                    }
                    if let Some(text) = &in_toast.text.1 {
                        let el = toast_doc.CreateElement(&hs("text"))?;
                        binding_el.AppendChild(&el)?;
                        text.write_to_element(2, &el)?;
                    }
                    if let Some(text) = &in_toast.text.2 {
                        let el = toast_doc.CreateElement(&hs("text"))?;
                        binding_el.AppendChild(&el)?;
                        text.write_to_element(3, &el)?;
                    }

                    for (id, image) in &in_toast.images {
                        let el = toast_doc.CreateElement(&hs("image"))?;
                        binding_el.AppendChild(&el)?;
                        image.write_to_element(*id, &el)?;
                    }
                }
            }
            // </binding>
        }
        // </visual>

        let toast = ToastNotification::CreateToastNotification(&toast_doc)?;

        if let Some(group) = &in_toast.group {
            toast.SetGroup(&hs(group))?;
        }
        if let Some(tag) = &in_toast.tag {
            toast.SetTag(&hs(tag))?;
        }
        if let Some(remote_id) = &in_toast.remote_id {
            toast.SetRemoteId(&hs(remote_id))?;
        }
        if let Some(exp) = in_toast.expires_in {
            let now = Calendar::new()?;
            now.AddSeconds(exp.as_secs() as i32)?;
            let dt = now.GetDateTime()?;
            toast.SetExpirationTime(&PropertyValue::CreateDateTime(dt)?.cast()?)?;
        }

        if let Some(mut dismissed) = on_dismissed {
            toast.Dismissed(&TypedEventHandler::new(
                move |_, args: &Option<ToastDismissedEventArgs>| {
                    if let Some(args) = args {
                        let arg = match args.Reason() {
                            Ok(r) => DismissalReason::from_winrt(r),
                            Err(e) => Err(e.into()),
                        };
                        dismissed(arg);
                    }
                    Ok(())
                },
            ))?;
        }

        if let Some(mut failed) = on_failed {
            toast.Failed(&TypedEventHandler::new(
                move |_, args: &Option<ToastFailedEventArgs>| {
                    if let Some(args) = args {
                        let e = args.ErrorCode().and_then(|e| e.ok());
                        if let Err(e) = e {
                            failed(e.into())
                        }
                    }
                    Ok(())
                },
            ))?;
        }

        if let Some(mut activated) = on_activated {
            toast.Activated(&TypedEventHandler::new(
                move |_, args: &Option<IInspectable>| {
                    let args = args
                        .as_ref()
                        .and_then(|arg| arg.cast::<ToastActivatedEventArgs>().ok());

                    if let Some(args) = args {
                        let arguments = args
                            .Arguments()
                            .map(|s| s.to_string_lossy())
                            .map_err(|e| e.into());
                        activated(arguments);
                    }

                    Ok(())
                },
            ))?;
        }

        notifier.Show(&toast)?;

        Ok(())
    }
}

/// Represents a Windows toast.
///
/// See <https://docs.microsoft.com/en-us/uwp/api/windows.ui.notifications.toastnotification>
/// 
/// # Example
/// ```rust
/// # use winrt_toast::{Toast, Text, Header};
/// # use winrt_toast::text::TextPlacement;
/// 
/// let mut toast = Toast::new()
///     .text1("Title")
///     .text2(Text::new("Body"))
///     .text3(
///         Text::new("Via SMS")
///             .with_placement(TextPlacement::Attribution)
///     );
/// ```
#[derive(Debug, Clone, Default)]
pub struct Toast {
    header: Option<Header>,
    text: (Option<Text>, Option<Text>, Option<Text>),
    images: HashMap<u8, Image>,
    tag: Option<String>,
    group: Option<String>,
    remote_id: Option<String>,
    expires_in: Option<Duration>,
}

impl Toast {
    /// Creates an empty toast.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a [`Header`] to this toast.
    pub fn header(&mut self, header: Header) -> &mut Toast {
        self.header = header.into();
        self
    }

    /// The first text element, usually the title.
    /// 
    /// # Example
    /// ```rust
    /// # use winrt_toast::{Toast, Text};
    /// # use winrt_toast::text::TextPlacement;
    /// # let mut toast = Toast::new();
    /// // You can use anything that is Into<String>
    /// toast.text1("text");
    /// 
    /// // Or you can use a `Text`
    /// toast.text1(
    ///     Text::new("text").with_placement(TextPlacement::Attribution)
    /// );
    /// ```
    pub fn text1<T: Into<Text>>(&mut self, text: T) -> &mut Toast {
        self.text.0 = Some(text.into());
        self
    }

    /// The second text element, usually the body.
    pub fn text2<T: Into<Text>>(&mut self, text: T) -> &mut Toast {
        self.text.1 = Some(text.into());
        self
    }

    /// The third text element, usually the body or attribution.
    pub fn text3<T: Into<Text>>(&mut self, text: T) -> &mut Toast {
        self.text.2 = Some(text.into());
        self
    }

    /// Add an image with the corresponding ID to the toast.
    ///
    /// # ID
    /// The image element in the toast template that this image is intended for.
    /// If a template has only one image, then this value is 1.
    /// The number of available image positions is based on the template definition.
    pub fn image(&mut self, id: u8, image: Image) -> &mut Toast {
        self.images.insert(id, image);
        self
    }

    /// Set the tag of this toast.
    ///
    /// See <https://docs.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast-cpp-uwp?tabs=xml#provide-a-primary-key-for-your-toast>
    pub fn tag(&mut self, tag: impl Into<String>) -> &mut Toast {
        self.tag = Some(tag.into());
        self
    }

    /// Set the group of this toast.
    ///
    /// See <https://docs.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast-cpp-uwp?tabs=xml#provide-a-primary-key-for-your-toast>
    pub fn group(&mut self, group: impl Into<String>) -> &mut Toast {
        self.group = Some(group.into());
        self
    }

    /// Set a remote id for the notification that enables the system to correlate
    /// this notification with another one generated on another device.
    pub fn remote_id(&mut self, remote_id: impl Into<String>) -> &mut Toast {
        self.remote_id = Some(remote_id.into());
        self
    }

    /// Set the expiration time of this toats, starting from the moment it is shown.
    ///
    /// After expiration, the toast will be removed from the Notification Center.
    pub fn expires_in(&mut self, duration: Duration) -> &mut Toast {
        self.expires_in = Some(duration);
        self
    }
}

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
    /// The given path is not absolute, and therefore cannot be converted to an URL.
    #[error("The given path is not absolute")]
    InvalidPath,
    /// The dismissal reason from OS is unknown
    #[error("The dismissal reason from OS is unknown")]
    InvalidDismissalReason,
}

/// The result type used in this crate.
pub type Result<T> = std::result::Result<T, WinToastError>;
