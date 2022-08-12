use windows::Data::Xml::Dom::XmlElement;

use crate::hs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextPlacement {
    Attribution,
}

impl TextPlacement {
    fn as_str(&self) -> &'static str {
        match self {
            TextPlacement::Attribution => "attribution",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Text {
    content: String,
    placement: Option<TextPlacement>,
}

impl Text {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            placement: None,
        }
    }

    pub fn with_placement(mut self, placement: TextPlacement) -> Self {
        self.placement = Some(placement);
        self
    }

    pub(crate) fn write_to_element(&self, id: u8, el: &XmlElement) -> crate::Result<()> {
        el.SetAttribute(&hs("id"), &hs(&format!("{}", id)))?;
        el.SetInnerText(&hs(self.content.to_string()))?;
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
