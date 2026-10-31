import ServiceManagement
import SwiftUI

/// "Launch at login", backed by macOS Login Items. Needs no permission: macOS
/// shows a one-time "Login item added" notice and lists Klippy under
/// System Settings → General → Login Items, where the user can turn it off.
@MainActor
final class LaunchAtLogin: ObservableObject {
    @Published private(set) var status = SMAppService.mainApp.status
    @Published private(set) var error: String?

    var isEnabled: Bool { status == .enabled }
    /// The user switched Klippy off in System Settings; only they can re-allow it.
    var needsApproval: Bool { status == .requiresApproval }

    func setEnabled(_ enabled: Bool) {
        do {
            if enabled {
                try SMAppService.mainApp.register()
            } else {
                try SMAppService.mainApp.unregister()
            }
            error = nil
        } catch {
            self.error = error.localizedDescription
        }
        refresh()
    }

    /// Re-read the status, e.g. when Settings opens, in case it changed in
    /// System Settings.
    func refresh() {
        status = SMAppService.mainApp.status
    }

    /// Turns it on the first time Klippy runs; after that the user's choice
    /// stands, even if they turned it off.
    static func enableOnFirstLaunch() {
        let key = "didSetUpLaunchAtLogin"
        // Login Items need a real .app bundle, not `swift run`.
        guard Bundle.main.bundleURL.pathExtension == "app",
              !UserDefaults.standard.bool(forKey: key)
        else { return }
        UserDefaults.standard.set(true, forKey: key)
        try? SMAppService.mainApp.register()
    }
}
