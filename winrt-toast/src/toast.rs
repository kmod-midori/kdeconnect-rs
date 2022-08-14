use std::{collections::HashMap, time::Duration};

use crate::{Header, Image, Text, Action};

/// Represents a Windows toast.
///
/// See <https://docs.microsoft.com/en-us/uwp/api/windows.ui.notifications.toastnotification>
///
#[derive(Debug, Clone, Default)]
pub struct Toast {
    pub(crate) header: Option<Header>,
    pub(crate) text: (Option<Text>, Option<Text>, Option<Text>),
    pub(crate) images: HashMap<u8, Image>,
    pub(crate) tag: Option<String>,
    pub(crate) group: Option<String>,
    pub(crate) remote_id: Option<String>,
    pub(crate) expires_in: Option<Duration>,
    pub(crate) scenario: Option<Scenario>,
    pub(crate) launch: Option<String>,
    pub(crate) duration: Option<ToastDuration>,
    pub(crate) actions: Vec<Action>,
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
    /// # use winrt_toast::content::text::TextPlacement;
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

    /// Add a new action to the toast.
    pub fn action(&mut self, action: Action) -> &mut Toast {
        self.actions.push(action);
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

    /// Set the scenario of this toast.
    ///
    /// The scenario adjusts a few behaviors to create a consistent and unified user experience.
    pub fn scenario(&mut self, scenario: Scenario) -> &mut Toast {
        self.scenario = Some(scenario);
        self
    }

    /// A string that is passed to the application when it is activated by the toast.
    ///
    /// The format and contents of this string are defined by the app for its own use.
    /// When the user taps or clicks the toast to launch its associated app,
    ///  the launch string provides the context to the app that allows it to show the user a view relevant to the toast content,
    /// rather than launching in its default way.
    pub fn launch(&mut self, launch: impl Into<String>) -> &mut Toast {
        self.launch = Some(launch.into());
        self
    }

    /// The amount of time the toast should display.
    pub fn duration(&mut self, duration: ToastDuration) -> &mut Toast {
        self.duration = Some(duration);
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

/// The scenario your toast is used for, like an alarm or reminder.
///
/// See [Microsoft documentation](https://docs.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/adaptive-interactive-toasts?tabs=xml#scenarios).
#[derive(Debug, Clone)]
pub enum Scenario {
    /// A reminder notification. This will be displayed pre-expanded and stay on the user's screen till dismissed.
    Reminder,
    /// An alarm notification. This will be displayed pre-expanded and stay on the user's screen till dismissed.
    /// Audio will loop by default and will use alarm audio.
    Alarm,
    /// An incoming call notification.
    /// This will be displayed pre-expanded in a special call format and stay on the user's screen till dismissed.
    /// Audio will loop by default and will use ringtone audio.
    IncomingCall,
    /// An important notification.
    /// This allows users to have more control over what apps can send them high-priority toast notifications that can break through Focus Assist (Do not Disturb).
    /// This can be modified in the notifications settings.
    Urgent,
}

impl Scenario {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Scenario::Reminder => "reminder",
            Scenario::Alarm => "alarm",
            Scenario::IncomingCall => "incomingCall",
            Scenario::Urgent => "urgent",
        }
    }
}

/// The amount of time the toast should display
#[derive(Debug, Clone)]
pub enum ToastDuration {
    /// The toast will display for a longer period of time.
    Long,
    /// The toast will display for a shorter period of time.
    Short,
}

impl ToastDuration {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ToastDuration::Long => "long",
            ToastDuration::Short => "short",
        }
    }
}
