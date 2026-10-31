import App
import AppKit
import SwiftUI

/// The menu bar ("tray") menu.
struct TrayMenu: View {
    @ObservedObject var core: Core
    @Environment(\.openWindow) private var openWindow

    /// Menu items don't truncate, so keep long copies to one short line.
    private static let titleLength = 50

    var body: some View {
        Button("Show Klippy") { show(KlippyApp.mainWindowID) }
        // Only once Touch ID has unlocked the history.
        if core.view.unlocked {
            Menu("History") {
                if core.view.recentItems.isEmpty {
                    Text("Nothing copied yet")
                }
                // The newest items only: menus build every row up front, so
                // the full history lives in the History window.
                ForEach(core.view.recentItems, id: \.id) { item in
                    Button(Self.title(item.preview)) { open(item) }
                }
                Divider()
                Button("Show All History…") { show(HistoryView.windowID) }
                if !core.view.failedItems.isEmpty {
                    Button("Failed to Save (\(core.view.failedItems.count))…") {
                        core.update(.setHistoryTab(.failed))
                        show(HistoryView.windowID)
                    }
                }
                Divider()
                Button("Clear History…") { show(ClearHistoryView.windowID) }
            }
        }
        Divider()
        Button("About Klippy", action: AboutPanel.show)
        SettingsLink {
            Text("Settings…")
        }
        .keyboardShortcut(",")
        Divider()
        Button("Quit Klippy") { NSApp.terminate(nil) }
            .keyboardShortcut("q")
    }

    /// Views or copies the item, as chosen in Settings.
    private func open(_ item: ClipItem) {
        switch core.view.trayClickAction {
        case .view:
            core.update(.setHistoryTab(.saved))
            core.update(.selectItem(item.id))
            show(HistoryView.windowID)
        case .copy:
            core.update(.copyItem(item.id))
        }
    }

    private func show(_ windowID: String) {
        openWindow(id: windowID)
        NSApp.activate(ignoringOtherApps: true)
    }

    private static func title(_ preview: String) -> String {
        preview.count > titleLength ? String(preview.prefix(titleLength - 1)) + "…" : preview
    }

    /// Paperclip outline from apple/Assets/tray.svg (Solar by 480 Design,
    /// https://creativecommons.org/licenses/by/4.0/). Embedded rather than
    /// bundled because the .app is assembled by hand without SwiftPM resources.
    /// A template image, so macOS tints it for light and dark menu bars.
    static let icon: NSImage = {
        let svg = """
        <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24">\
        <path fill="none" stroke="black" stroke-linecap="round" stroke-width="1.5" \
        d="m12.95 11.009l-2.94 2.926a4.125 4.125 0 0 0 0 5.853a4.17 4.17 0 0 0 5.88 0l3.675-3.658\
        a8.25 8.25 0 0 0 0-11.706c-3.248-3.232-8.512-3.232-11.76 0L4.132 8.082a7.22 7.22 0 0 0 0 10.243"/>\
        </svg>
        """
        let image = NSImage(data: Data(svg.utf8)) ?? NSImage(systemSymbolName: "paperclip", accessibilityDescription: nil)!
        image.size = NSSize(width: 18, height: 18)
        image.isTemplate = true
        image.accessibilityDescription = "Klippy"
        return image
    }()
}
