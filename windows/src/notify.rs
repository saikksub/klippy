//! Toast notifications. An unpackaged app needs an App User Model ID that
//! Windows knows about, so Klippy registers one for the current user.

use std::{os::windows::process::CommandExt, path::Path, process::Command};

use tauri_winrt_notification::Toast;
use windows::{
    UI::Notifications::{NotificationSetting, ToastNotificationManager},
    Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID,
    core::HSTRING,
};

use crate::registry;

const APP_ID: &str = "com.kksaisub.klippy";

/// Names Klippy's toasts and gives them its icon.
pub fn register(icon: Option<&Path>) {
    let key = format!(r"Software\Classes\AppUserModelId\{APP_ID}");
    let _ = registry::set_string(&key, "DisplayName", "Klippy");
    if let Some(icon) = icon {
        let _ = registry::set_string(&key, "IconUri", &icon.display().to_string());
    }
    // SAFETY: plain call with a valid string.
    let _ = unsafe { SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(APP_ID)) };
}

/// Windows doesn't prompt; notifications are on unless the user turned them
/// off in Settings → System → Notifications.
pub fn allowed() -> bool {
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))
        .and_then(|notifier| notifier.Setting())
        .is_ok_and(|setting| setting == NotificationSetting::Enabled)
}

/// Clicking a toast about a saved file (or its button) shows the file in
/// Explorer.
pub fn show(title: &str, body: &str, reveal_path: Option<String>) {
    let mut toast = Toast::new(APP_ID).title(title).text1(body);
    if let Some(path) = reveal_path {
        toast = toast
            .add_button("Show in folder", "reveal")
            .on_activated(move |_| {
                reveal(&path);
                Ok(())
            });
    }
    if let Err(error) = toast.show() {
        eprintln!("klippy: notification: {error}");
    }
}

/// Opens Explorer with the file selected, or the folder it was saved in if
/// it has been moved or deleted since.
pub fn reveal(path: &str) {
    let path = Path::new(path);
    let result = if path.exists() {
        // Explorer wants the quotes after the comma, so build the argument by hand.
        Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .spawn()
    } else if let Some(folder) = path.parent() {
        Command::new("explorer").arg(folder).spawn()
    } else {
        return;
    };
    if let Err(error) = result {
        eprintln!("klippy: explorer: {error}");
    }
}

/// Opens a Settings page (`ms-settings:…`) or a website in the default browser.
pub fn open_uri(uri: &str) {
    if let Err(error) = Command::new("explorer").arg(uri).spawn() {
        eprintln!("klippy: open {uri}: {error}");
    }
}
