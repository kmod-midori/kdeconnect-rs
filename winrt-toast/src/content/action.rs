use windows::Data::Xml::Dom::XmlElement;

use crate::hs;

/// Specifies a button shown in a toast.
#[derive(Debug, Clone)]
pub struct Action {
    content: String,
    arguments: String,
    r#type: String,
    activation_type: Option<ActivationType>,
    placement: Option<ActionPlacement>,
}

impl Action {
    /// Create a new action.
    ///
    /// `arguments`: An argument string that can be passed to the associated app
    /// to provide specifics about the action that it should execute
    /// in response to the user action.
    /// 
    /// `typ`: An argument string that can be passed to the associated app to 
    /// provide specifics about the action that it should execute in response to the user action.
    pub fn new(
        content: impl Into<String>,
        arguments: impl Into<String>,
        typ: impl Into<String>,
    ) -> Self {
        Self {
            content: content.into(),
            arguments: arguments.into(),
            r#type: typ.into(),
            activation_type: None,
            placement: None,
        }
    }

    /// The activation type of the action.
    pub fn with_activation_type(mut self, activation_type: ActivationType) -> Self {
        self.activation_type = Some(activation_type);
        self
    }

    /// The placement of the action.
    pub fn with_placement(mut self, placement: ActionPlacement) -> Self {
        self.placement = Some(placement);
        self
    }

    pub(crate) fn write_to_element(&self, el: &XmlElement) -> crate::Result<()> {
        el.SetAttribute(&hs("content"), &hs(&self.content))?;
        el.SetAttribute(&hs("arguments"), &hs(&self.arguments))?;
        el.SetAttribute(&hs("type"), &hs(&self.r#type))?;
        if let Some(activation_type) = self.activation_type {
            el.SetAttribute(&hs("activationType"), &hs(activation_type.as_str()))?;
        }
        if let Some(placement) = self.placement {
            el.SetAttribute(&hs("placement"), &hs(placement.as_str()))?;
        }

        Ok(())
    }
}

/// The type of activation that will be used when the user interacts with a specific action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationType {
    /// Default value. Your foreground app is launched.
    Foreground,
    /// Your corresponding background task is triggered, and you can execute code in the background without interrupting the user.
    Background,
    /// Launch a different app using protocol activation.
    Protocol,
}

impl ActivationType {
    fn as_str(&self) -> &'static str {
        match self {
            ActivationType::Foreground => "foreground",
            ActivationType::Background => "background",
            ActivationType::Protocol => "protocol",
        }
    }
}

/// The location of the action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionPlacement {
    /// The action becomes a context menu action added to the toast notification's
    /// context menu rather than a traditional toast button.
    ContextMenu,
}

impl ActionPlacement {
    fn as_str(&self) -> &'static str {
        match self {
            ActionPlacement::ContextMenu => "contextMenu",
        }
    }
}