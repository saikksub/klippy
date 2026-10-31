import SwiftUI

/// Safari-style settings: toolbar tabs over a fixed, roughly square window.
struct SettingsView: View {
    @ObservedObject var core: Core
    @ObservedObject var launchAtLogin: LaunchAtLogin

    var body: some View {
        TabView {
            GeneralSettingsView(core: core, launchAtLogin: launchAtLogin)
                .tabItem { Label("General", systemImage: "gearshape") }
            AboutSettingsView()
                .tabItem { Label("About", systemImage: "info.circle") }
        }
        // The tab toolbar adds roughly 80pt above this, making the window square.
        .frame(width: 420, height: 340)
    }
}
