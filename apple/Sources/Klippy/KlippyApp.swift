import SwiftUI
import UserNotifications

@main
struct KlippyApp: App {
    static let mainWindowID = "main"

    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var launchAtLogin = LaunchAtLogin()

    /// Shared by every window; owned by the app delegate so it can check for
    /// failed copies before quitting.
    private var core: Core { appDelegate.core }

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
        .commands {
            CommandGroup(replacing: .appInfo) {
                Button("About Klippy", action: AboutPanel.show)
            }
            CommandGroup(after: .appSettings) {
                ClearHistoryMenuItem(core: core)
            }
        }

        // Resizable, can go full screen; one instance, opened from a row's →.
        Window("History", id: HistoryView.windowID) {
            HistoryView(core: core)
        }
        .defaultSize(width: 900, height: 600)

        // The type-"confirm" dialog; opened from menus and Settings only.
        Window("Clear History", id: ClearHistoryView.windowID) {
            ClearHistoryView(core: core)
        }
        .windowResizability(.contentSize)
        .commandsRemoved()

        MenuBarExtra {
            TrayMenu(core: core)
        } label: {
            Image(nsImage: TrayMenu.icon)
        }

        // Gets the app menu's "Settings…" item and its ⌘, shortcut.
        Settings {
            SettingsView(core: core, launchAtLogin: launchAtLogin)
        }
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, UNUserNotificationCenterDelegate {
    let core = Core(storePath: Core.defaultStorePath())

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Needed so `swift run` (an unbundled executable) gets a Dock icon and key window.
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        LaunchAtLogin.enableOnFirstLaunch()
        if Bundle.main.bundleURL.pathExtension == "app" {
            UNUserNotificationCenter.current().delegate = self
        }
    }

    /// Failed copies live only in memory, so warn before they're discarded.
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        let failed = core.view.failedItems.count
        guard failed > 0 else { return .terminateNow }
        let them = failed == 1 ? "it" : "them"
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = failed == 1 ? "1 copy failed to save" : "\(failed) copies failed to save"
        alert.informativeText = "Klippy found malicious content in \(them) and didn't save \(them). "
            + "Failed copies are kept only while Klippy is running and will be discarded when it quits."
        alert.addButton(withTitle: "Quit")
        alert.addButton(withTitle: "Cancel")
        NSApp.activate(ignoringOtherApps: true)
        return alert.runModal() == .alertFirstButtonReturn ? .terminateNow : .terminateCancel
    }

    /// Show Klippy's notifications even while it's the frontmost app.
    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        [.banner, .sound]
    }

    /// Keep running in the menu bar after the windows are closed.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }
}
