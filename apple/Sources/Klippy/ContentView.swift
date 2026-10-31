import App
import SwiftUI

/// Shows the page the Rust core says is current.
struct ContentView: View {
    @ObservedObject var core: Core

    var body: some View {
        switch core.view.page {
        case .splash:
            SplashView()
                .task { core.update(.start) }
        case .locked:
            LockedView(core: core)
        case .setup:
            SetupView()
        case .home:
            HomeView(core: core)
                // Only watch once access is confirmed: polling without it
                // would make macOS prompt on every read.
                .task { core.watchClipboard() }
        }
    }
}
