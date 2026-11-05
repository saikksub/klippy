//! The Win32 clipboard. Windows formats are mapped to the macOS pasteboard
//! type names the core expects (`public.html`, `public.rtf`), so the core
//! stays the same on both platforms.

use std::num::NonZeroU32;

use clipboard_win::{Clipboard, options::NoClear, raw};
use shared::{ClipboardContent, ClipboardFormat};

const HTML_KIND: &str = "public.html";
const RTF_KIND: &str = "public.rtf";

/// The clipboard's change counter. Reading it doesn't open the clipboard, so
/// ordinary copies never touch its content.
pub fn sequence_number() -> u32 {
    raw::seq_num().map_or(0, NonZeroU32::get)
}

/// The clipboard's text with its HTML and RTF versions, or `None` if it has
/// no text or the source app asked clipboard tools to skip it. Password
/// managers set those markers on passwords; see
/// <https://learn.microsoft.com/windows/win32/dataxchg/clipboard-formats#cloud-clipboard-and-clipboard-history-formats>.
pub fn read() -> Option<ClipboardContent> {
    let _open = Clipboard::new_attempts(10).ok()?;
    if is_private() {
        return None;
    }
    let mut text = Vec::new();
    raw::get_string(&mut text).ok()?;
    let mut text = String::from_utf8(text).ok()?;
    text.truncate(text.trim_end_matches('\0').len());

    let mut formats = Vec::new();
    if let Some(html) = format_id("HTML Format").filter(|&id| raw::is_format_avail(id)) {
        // Only the copied fragment, as on macOS; the header and the source
        // page's URL are dropped.
        let mut data = Vec::new();
        if raw::get_html(html, &mut data).is_ok() {
            formats.push(ClipboardFormat {
                kind: HTML_KIND.into(),
                data,
            });
        }
    }
    if let Some(rtf) = format_id("Rich Text Format").filter(|&id| raw::is_format_avail(id)) {
        let mut data = Vec::new();
        if raw::get_vec(rtf, &mut data).is_ok() {
            data.truncate(data.iter().rposition(|&byte| byte != 0).map_or(0, |last| last + 1));
            formats.push(ClipboardFormat {
                kind: RTF_KIND.into(),
                data,
            });
        }
    }
    Some(ClipboardContent { text, formats })
}

fn is_private() -> bool {
    let present = |name| format_id(name).is_some_and(raw::is_format_avail);
    if present("ExcludeClipboardContentFromMonitorProcessing") || present("Clipboard Viewer Ignore") {
        return true;
    }
    // A DWORD 0 here means "keep out of clipboard history".
    let mut data = Vec::new();
    format_id("CanIncludeInClipboardHistory")
        .filter(|&id| raw::is_format_avail(id))
        .is_some_and(|id| {
            raw::get_vec(id, &mut data).is_ok() && data.get(..4) == Some(&[0, 0, 0, 0][..])
        })
}

/// Puts back the text and every format Windows can take; formats from other
/// systems are skipped.
pub fn write(content: &ClipboardContent) -> Result<(), String> {
    let _open = Clipboard::new_attempts(10).map_err(|error| error.to_string())?;
    raw::empty().map_err(|error| error.to_string())?;
    raw::set_string_with(&content.text, NoClear).map_err(|error| error.to_string())?;
    for format in &content.formats {
        let result = match format.kind.as_str() {
            HTML_KIND => match (format_id("HTML Format"), std::str::from_utf8(&format.data)) {
                (Some(id), Ok(html)) => raw::set_html(id, html),
                _ => Ok(()),
            },
            RTF_KIND => match format_id("Rich Text Format") {
                Some(id) => raw::set_without_clear(id, &format.data),
                None => Ok(()),
            },
            _ => Ok(()),
        };
        result.map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn format_id(name: &str) -> Option<u32> {
    raw::register_format(name).map(NonZeroU32::get)
}
