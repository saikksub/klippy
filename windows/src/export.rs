//! Saves exports the Rust core prepared: HTML and Markdown as they are, PDF
//! by laying the core's (already sanitised) HTML out into pages with
//! WebView2's PDF printer.

use std::{
    num::NonZeroIsize,
    path::{Path, PathBuf},
};

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle, Win32WindowHandle,
    WindowHandle,
};
use shared::{ExportFormat, SaveExport};
use webview2_com::{
    Microsoft::Web::WebView2::Win32::{ICoreWebView2_2, ICoreWebView2_7, ICoreWebView2Environment6},
    PrintToPdfCompletedHandler,
};
use windows::core::{HSTRING, Interface};
use wry::{WebView, WebViewExtWindows};

/// A save dialog, or a free name in Downloads for quick exports. Blocks
/// while the dialog is open, so call it off the UI thread.
pub fn destination(export: &SaveExport, owner: isize) -> Option<PathBuf> {
    if !export.ask_location {
        return Some(unique_path(&dirs::download_dir()?, &export.file_name));
    }
    let (label, extension) = match export.format {
        ExportFormat::Pdf => ("PDF", "pdf"),
        ExportFormat::Html => ("HTML page", "html"),
        ExportFormat::Markdown => ("Markdown", "md"),
    };
    let mut dialog = rfd::FileDialog::new()
        .set_file_name(&export.file_name)
        .add_filter(label, &[extension]);
    if let Some(owner) = Owner::new(owner) {
        dialog = dialog.set_parent(&owner);
    }
    dialog.save_file()
}

pub fn write(path: &Path, data: &[u8]) -> Result<(), String> {
    std::fs::write(path, data).map_err(|error| error.to_string())
}

/// "Name.pdf", or "Name 2.pdf" etc. if that's taken.
fn unique_path(folder: &Path, name: &str) -> PathBuf {
    let base = folder.join(name);
    let stem = base.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
    let extension = base.extension().map(|ext| ext.to_string_lossy().into_owned()).unwrap_or_default();
    let mut path = base;
    let mut number = 2;
    while path.exists() {
        path = folder.join(format!("{stem} {number}.{extension}"));
        number += 1;
    }
    path
}

/// Prints the page loaded in `webview` to `path`; `done` gets whether it
/// worked, on the UI thread.
pub fn print_pdf(
    webview: &WebView,
    path: &Path,
    done: impl FnOnce(bool) + 'static,
) -> windows::core::Result<()> {
    let webview: ICoreWebView2_7 = webview.webview().cast()?;
    // SAFETY: COM calls on the UI thread that owns the webview.
    unsafe {
        let environment: ICoreWebView2Environment6 =
            webview.cast::<ICoreWebView2_2>()?.Environment()?.cast()?;
        let settings = environment.CreatePrintSettings()?;
        settings.SetShouldPrintBackgrounds(true)?;
        settings.SetShouldPrintHeaderAndFooter(false)?;
        // Inches; the macOS shell uses 48pt.
        settings.SetMarginTop(0.67)?;
        settings.SetMarginBottom(0.67)?;
        settings.SetMarginLeft(0.67)?;
        settings.SetMarginRight(0.67)?;
        let handler = PrintToPdfCompletedHandler::create(Box::new(move |result, succeeded| {
            done(result.is_ok() && succeeded);
            Ok(())
        }));
        webview.PrintToPdf(&HSTRING::from(path.as_os_str()), &settings, &handler)
    }
}

/// The window that owns the save dialog, passed across threads as its HWND.
struct Owner(NonZeroIsize);

impl Owner {
    fn new(hwnd: isize) -> Option<Self> {
        NonZeroIsize::new(hwnd).map(Self)
    }
}

impl HasWindowHandle for Owner {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: the HWND belongs to a window that outlives the dialog.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(Win32WindowHandle::new(self.0))) })
    }
}

impl HasDisplayHandle for Owner {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::windows())
    }
}
