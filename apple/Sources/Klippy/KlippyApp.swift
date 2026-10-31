import SwiftUI

@main
struct KlippyApp: App {
    static let mainWindowID = "main"

    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    /// Shared by the main and Settings windows.
    @StateObject private var core = Core()
    @StateObject private var launchAtLogin = LaunchAtLogin()

    var body: some Scene {
        // A single window (like `History`), so the tray's "Show Klippy" brings
        // it back instead of opening another copy.
        Window("Klippy", id: Self.mainWindowID) {
            ContentView(core: core)
                .frame(width: 300, height: 480)
        }
        // Fixed, compact window like classic Skype: the window takes the
        // content's fixed size, so it can't be resized or zoomed.
        .windowResizability(.contentSize)

        // Resizable, can go full screen; one instance, opened from a row's →.
        Window("History", id: HistoryView.windowID) {
            HistoryView(core: core)
        }
        .defaultSize(width: 900, height: 600)

        MenuBarExtra {
            TrayMenu()
        } label: {
            Image(nsImage: TrayMenu.icon)
        }

        // Gets the app menu's "Settings…" item and its ⌘, shortcut.
        Settings {
            SettingsView(core: core, launchAtLogin: launchAtLogin)
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        // Needed so `swift run` (an unbundled executable) gets a Dock icon and key window.
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        LaunchAtLogin.enableOnFirstLaunch()
    }

    /// Keep running in the menu bar after the windows are closed.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }
}
