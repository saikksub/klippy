//! Klippy for Windows: a tray app that runs the same Rust core as the macOS
//! shell, with the Win32 clipboard in place of `NSPasteboard`. The windows
//! are WebView2 pages (see `ui/`).

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod clipboard;
#[cfg(windows)]
mod export;
#[cfg(windows)]
mod icon;
#[cfg(windows)]
mod images;
#[cfg(windows)]
mod instance;
#[cfg(windows)]
mod login;
#[cfg(windows)]
mod notify;
#[cfg(windows)]
mod registry;
#[cfg(windows)]
mod shell;
#[cfg(windows)]
mod tray;
#[cfg(windows)]
mod ui;

#[cfg(windows)]
fn main() {
    shell::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("klippy-windows only runs on Windows; on macOS, use the SwiftUI shell in apple/.");
    std::process::exit(1);
}
