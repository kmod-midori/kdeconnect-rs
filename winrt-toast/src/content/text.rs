use windows::Data::Xml::Dom::XmlElement;

use crate::hs;

/// The placement of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextPlacement {
    /// Introduced in Anniversary Update.
    ///
    /// If you specify the value "attribution", the text is always displayed at the bottom of your notification,
    /// along with your app's identity or the notification's timestamp.
    ///
    /// On older versions of Windows that don't support attribution text,
    /// the text will simply be displayed as another text element.
    Attribution,
}

impl TextPlacement {
    fn as_str(&self) -> &'static str {
        match self {
            TextPlacement::Attribution => "attribution",
        }
    }
}

/// Specifies text used in the toast template.
#[derive(Debug, Clone)]
pub struct Text {
    content: String,
    placement: Option<TextPlacement>,
}

impl Text {
    /// Create a new text element.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            placement: None,
        }
    }

    /// The placement of the text.
    pub fn with_placement(mut self, placement: TextPlacement) -> Self {
        self.placement = Some(placement);
        self
    }

    /// Set the placement of the text to [`TextPlacement::Attribution`].
    pub fn as_attribution(self) -> Self {
        self.with_placement(TextPlacement::Attribution)
    }

    pub(crate) fn write_to_element(&self, id: u8, el: &XmlElement) -> crate::Result<()> {
        el.SetAttribute(&hs("id"), &hs(&format!("{}", id)))?;
        el.SetInnerText(&hs(&self.content))?;
        if let Some(placement) = self.placement {
            el.SetAttribute(&hs("placement"), &hs(placement.as_str()))?;
        }

        Ok(())
    }
}

impl<T> From<T> for Text
where
    T: Into<String>,
{
    fn from(content: T) -> Self {
        Self::new(content)
    }
}
