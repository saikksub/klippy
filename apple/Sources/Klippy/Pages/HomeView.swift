import App
import SwiftUI

struct HomeView: View {
    @ObservedObject var core: Core
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        VStack(spacing: 0) {
            searchBar
            if !searchTerm.isEmpty {
                searchInfo
            }
            Divider()
            history
            Divider()
            bottomBar
        }
    }

    private var searchBar: some View {
        TextField("Search", text: Binding(
            get: { core.view.search },
            set: { core.update(.search($0)) }
        ))
        .textFieldStyle(.roundedBorder)
        .padding(8)
    }

    /// The search as the core matches it (surrounding spaces ignored).
    private var searchTerm: String {
        core.view.search.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var searchInfo: some View {
        let count = core.view.items.count
        return VStack(alignment: .leading, spacing: 2) {
            Text("You are searching for \"\(searchTerm)\"")
                .lineLimit(1)
                .truncationMode(.middle)
            Text("Found \(count) \(count == 1 ? "result" : "results")")
                .foregroundStyle(.secondary)
        }
        .font(.callout)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(Color.accentColor.opacity(0.15))
        )
        .padding(.horizontal, 8)
        .padding(.bottom, 8)
    }

    /// `List` is backed by a reusing table view, so only visible rows are
    /// rendered however long the history gets.
    private var history: some View {
        List {
            ForEach(core.view.items, id: \.id) { item in
                row(item)
            }
            if !core.view.items.isEmpty {
                Text("End of the list")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                    .frame(maxWidth: .infinity)
                    .padding(.top, 8)
                    .listRowSeparator(.hidden)
                    .listRowBackground(Color.clear)
            }
        }
        .listStyle(.plain)
        .scrollContentBackground(.hidden)
        .contentMargins(.top, 6, for: .scrollContent)
        .contentMargins(.bottom, 16, for: .scrollContent)
        .overlay {
            // While searching, the info box above already says nothing was found.
            if core.view.items.isEmpty && searchTerm.isEmpty {
                VStack(spacing: 8) {
                    triggerBadge(.title3)
                    Text("to save a copy")
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    private func row(_ item: ClipItem) -> some View {
        ClipRow(
            item: item,
            onTap: { core.update(.copyItem(item.id)) },
            onCopy: { core.update(.copyItem(item.id)) },
            onOpen: {
                core.update(.selectItem(item.id))
                openWindow(id: HistoryView.windowID)
            }
        )
        .listRowSeparator(.hidden)
        .listRowBackground(Color.clear)
        .listRowInsets(EdgeInsets(top: 3, leading: 8, bottom: 3, trailing: 8))
    }

    /// "⌘C × 3": the shortcut as a code-style key, times the configured count.
    /// Used in the bottom bar and, larger, when the list is empty.
    private func triggerBadge(_ style: Font.TextStyle) -> some View {
        HStack(spacing: 5) {
            Text("⌘C")
                .font(.system(style, design: .monospaced).weight(.medium))
                .padding(.horizontal, 5)
                .padding(.vertical, 2)
                .background(
                    RoundedRectangle(cornerRadius: 4, style: .continuous)
                        .fill(Color.primary.opacity(0.1))
                )
            Text("× \(core.view.copyTrigger)")
                .font(.system(style))
        }
        .foregroundStyle(.secondary)
        .help(core.view.copyTriggerHint)
    }

    private var bottomBar: some View {
        HStack {
            triggerBadge(.caption)
            Spacer()
            Menu {
                SettingsLink {
                    Text("Settings…")
                }
                .keyboardShortcut(",")
                Divider()
                Button("Quit Klippy") { NSApp.terminate(nil) }
                    .keyboardShortcut("q")
            } label: {
                Image(systemName: "gearshape")
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
    }
}
