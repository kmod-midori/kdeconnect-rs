use std::path::Path;

use url::Url;
use windows::Data::Xml::Dom::XmlElement;

use crate::hs;

/// The placement of the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePlacement {
    /// The image replaces your app's logo in the toast notification.
    AppLogoOverride,
    /// The image is displayed as a hero image.
    Hero,
}

impl ImagePlacement {
    fn as_str(&self) -> &'static str {
        match self {
            ImagePlacement::AppLogoOverride => "appLogoOverride",
            ImagePlacement::Hero => "hero",
        }
    }
}

/// The cropping of the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageHintCrop {
    Circle,
}

impl ImageHintCrop {
    fn as_str(&self) -> &'static str {
        match self {
            ImageHintCrop::Circle => "circle",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Image {
    src: url::Url,
    placement: Option<ImagePlacement>,
    hint_crop: Option<ImageHintCrop>,
    alt: Option<String>,
}

impl Image {
    /// Create an [`Image`] from a [`Url`].
    pub fn new(src: Url) -> Self {
        Self {
            src,
            placement: None,
            hint_crop: None,
            alt: None,
        }
    }

    /// Create an [`Image`] from a local path.
    /// 
    /// This will return `Err` if the path is not absolute.
    pub fn new_local(path: impl AsRef<Path>) -> crate::Result<Self> {
        let url = Url::from_file_path(path).map_err(|_| crate::WinToastError::InvalidPath)?;
        Ok(Self::new(url))
    }

    /// The placement of the image.
    pub fn with_placement(mut self, placement: ImagePlacement) -> Self {
        self.placement = Some(placement);
        self
    }

    /// The cropping of the image.
    pub fn with_hint_crop(mut self, crop: ImageHintCrop) -> Self {
        self.hint_crop = Some(crop);
        self
    }

    pub fn with_alt(mut self, alt: impl Into<String>) -> Self {
        self.alt = Some(alt.into());
        self
    }

    pub(crate) fn write_to_element(&self, id: u8, el: &XmlElement) -> crate::Result<()> {
        el.SetAttribute(&hs("id"), &hs(&format!("{}", id)))?;
        el.SetAttribute(&hs("src"), &hs(self.src.to_string()))?;
        if let Some(placement) = self.placement {
            el.SetAttribute(&hs("placement"), &hs(placement.as_str()))?;
        }
        if let Some(crop) = self.hint_crop {
            el.SetAttribute(&hs("hint-crop"), &hs(crop.as_str()))?;
        }
        if let Some(alt) = &self.alt {
            el.SetAttribute(&hs("alt"), &hs(alt))?;
        }

        Ok(())
    }
}
