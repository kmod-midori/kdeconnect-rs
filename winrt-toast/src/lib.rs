pub mod text;
use std::collections::HashMap;

pub use text::Text;
pub mod image;
pub use image::Image;
pub mod header;
pub use header::Header;

/// Re-export of the `url` crate.
pub use url;
use windows::{
    core::{HSTRING, IInspectable},
    Data::Xml::Dom::XmlDocument,
    Foundation::TypedEventHandler,
    UI::Notifications::{
        ToastDismissalReason, ToastDismissedEventArgs, ToastFailedEventArgs, ToastNotification,
        ToastNotificationManager, ToastActivatedEventArgs,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DismissalReason {
    UserCanceled,
    ApplicationHidden,
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

pub struct ToastManager {
    app_id: HSTRING,
}

impl ToastManager {
    pub fn show<D, F>(
        &self,
        toast: &Toast,
        on_dismissed: Option<D>,
        on_failed: Option<F>,
    ) -> Result<()>
    where
        D: FnMut(Result<DismissalReason>) -> () + Send + 'static,
        F: FnMut(WinToastError) -> () + Send + 'static,
    {
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&self.app_id)?;

        let toast_doc = XmlDocument::new()?;

        let toast_el = toast_doc.CreateElement(&hs("toast"))?;
        toast_doc.AppendChild(&toast_el)?;

        // <header>
        if let Some(header) = &toast.header {
            let el = toast_doc.CreateElement(&hs("header"))?;
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
                    if let Some(text) = &toast.text.0 {
                        let el = toast_doc.CreateElement(&hs("text"))?;
                        binding_el.AppendChild(&el)?;
                        text.write_to_element(1, &el)?;
                    }
                    if let Some(text) = &toast.text.1 {
                        let el = toast_doc.CreateElement(&hs("text"))?;
                        binding_el.AppendChild(&el)?;
                        text.write_to_element(2, &el)?;
                    }
                    if let Some(text) = &toast.text.2 {
                        let el = toast_doc.CreateElement(&hs("text"))?;
                        binding_el.AppendChild(&el)?;
                        text.write_to_element(3, &el)?;
                    }

                    for (id, image) in &toast.images {
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

        // toast.Activated(&TypedEventHandler::new(move |_, args: &Option<IInspectable>| {
        //     Ok(())
        // }))?;

        notifier.Show(&toast)?;

        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct Toast {
    header: Option<Header>,
    text: (Option<Text>, Option<Text>, Option<Text>),
    images: HashMap<u8, Image>,
    tag: Option<String>,
    group: Option<String>,
}

impl Toast {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a [`Header`] to this toast.
    pub fn header(&mut self, header: Header) -> &mut Toast {
        self.header = header.into();
        self
    }

    /// The first text element, usually the title.
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
    /// ### ID
    /// The image element in the toast template that this image is intended for.
    /// If a template has only one image, then this value is 1.
    /// The number of available image positions is based on the template definition.
    pub fn image(&mut self, id: u8, image: Image) -> &mut Toast {
        self.images.insert(id, image);
        self
    }

    /// Set the tag of the toast.
    ///
    /// See https://docs.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast-cpp-uwp?tabs=xml#provide-a-primary-key-for-your-toast
    pub fn tag(&mut self, tag: impl Into<String>) -> &mut Toast {
        self.tag = Some(tag.into());
        self
    }

    /// Set the group of the toast.
    ///
    /// See https://docs.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast-cpp-uwp?tabs=xml#provide-a-primary-key-for-your-toast
    pub fn group(&mut self, group: impl Into<String>) -> &mut Toast {
        self.group = Some(group.into());
        self
    }
}

/// Convert a string to a HSTRING
pub(crate) fn hs(s: impl AsRef<str>) -> HSTRING {
    let s = s.as_ref();
    HSTRING::from(s)
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WinToastError {
    #[error("Windows API error: {0}")]
    Os(#[from] windows::core::Error),
    #[error("The given path is not absolute")]
    InvalidPath,
    #[error("The dismissal reason from OS is unknown")]
    InvalidDismissalReason,
}

pub type Result<T> = std::result::Result<T, WinToastError>;
