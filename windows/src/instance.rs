//! Keeps Klippy to one instance per user: launching it again shows the
//! running copy's main window instead of starting a second clipboard watcher
//! on the same history.

use windows::{
    Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HANDLE},
        System::Threading::{CreateEventW, CreateMutexW, INFINITE, SetEvent, WaitForSingleObject},
    },
    core::HSTRING,
};

/// `true` if this is the only instance; `on_show` then runs (on a background
/// thread) whenever another launch asks for the window. Otherwise the
/// running instance has been asked to show itself and this one should exit.
pub fn claim(on_show: impl Fn() + Send + 'static) -> bool {
    // SAFETY: plain calls with valid names; the handles live for the process.
    let Ok(show) = (unsafe { CreateEventW(None, false, false, &HSTRING::from(r"Local\com.kksaisub.klippy.show")) })
    else {
        return true;
    };
    let Ok(_mutex) = (unsafe { CreateMutexW(None, true, &HSTRING::from(r"Local\com.kksaisub.klippy")) }) else {
        return true;
    };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        let _ = unsafe { SetEvent(show) };
        return false;
    }
    let show = show.0 as isize;
    std::thread::spawn(move || {
        loop {
            // SAFETY: the event handle is never closed.
            unsafe { WaitForSingleObject(HANDLE(show as *mut _), INFINITE) };
            on_show();
        }
    });
    true
}
