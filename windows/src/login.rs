//! "Launch at login", backed by the per-user `Run` key. Needs no permission;
//! Windows lists Klippy under Settings → Apps → Startup (and Task Manager's
//! Startup apps), where the user can turn it off.

use crate::registry;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// Where Windows records that the user switched an app off in Startup apps.
const APPROVED_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const VALUE: &str = "Klippy";
const APP_KEY: &str = r"Software\Klippy";

pub fn is_enabled() -> bool {
    registry::get_string(RUN_KEY, VALUE).is_some() && !needs_approval()
}

/// The user switched Klippy off in Startup apps; only they can turn it back
/// on there. Windows marks that with an odd first byte.
pub fn needs_approval() -> bool {
    registry::get_string(RUN_KEY, VALUE).is_some()
        && registry::get_binary(APPROVED_KEY, VALUE)
            .and_then(|data| data.first().copied())
            .is_some_and(|flag| flag % 2 == 1)
}

pub fn set_enabled(enabled: bool) -> Result<(), String> {
    let result = if enabled {
        let exe = std::env::current_exe().map_err(|error| error.to_string())?;
        registry::set_string(RUN_KEY, VALUE, &format!("\"{}\"", exe.display()))
    } else {
        registry::delete(RUN_KEY, VALUE)
    };
    result.map_err(|error| error.message())
}

/// Turns it on the first time Klippy runs; after that the user's choice
/// stands, even if they turned it off.
pub fn enable_on_first_launch() {
    const KEY: &str = "DidSetUpLaunchAtLogin";
    if registry::get_dword(APP_KEY, KEY).is_some() {
        return;
    }
    if registry::set_dword(APP_KEY, KEY, 1).is_ok() {
        let _ = set_enabled(true);
    }
}
