use std::collections::HashSet;

use anyhow::Result;
use clipboard_win::{formats, Clipboard, Getter, Setter};

#[derive(Debug)]
pub enum ClipboardContent {
    Text(String),
    Files(Vec<String>),
    Unsupported,
}

/// Attempt to open (and lock) the global clipboard with a 100ms attempt timeout.
fn try_open_clipboard() -> Result<Clipboard> {
    let mut clipboard = None;
    for _ in 0..10 {
        if let Ok(c) = Clipboard::new() {
            clipboard = Some(c);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    if let Some(c) = clipboard {
        Ok(c)
    } else {
        Err(anyhow::anyhow!("Could not open clipboard"))
    }
}

pub fn read() -> Result<ClipboardContent> {
    let _clip = try_open_clipboard()?;

    let formats = clipboard_win::EnumFormats::new().collect::<HashSet<_>>();

    if formats.contains(&formats::CF_UNICODETEXT) {
        let mut text = String::new();
        formats::Unicode.read_clipboard(&mut text)?;
        return Ok(ClipboardContent::Text(text));
    }

    if formats.contains(&formats::CF_HDROP) {
        let mut list: Vec<String> = vec![];
        formats::FileList.read_clipboard(&mut list)?;
        return Ok(ClipboardContent::Files(list));
    }

    Ok(ClipboardContent::Unsupported)
}

pub fn write(content: ClipboardContent) -> Result<()> {
    let _clip = try_open_clipboard()?;

    match content {
        ClipboardContent::Text(s) => {
            formats::Unicode.write_clipboard(&s)?;
        }
        _ => anyhow::bail!("This format is currently not supported for write"),
    }

    Ok(())
}
