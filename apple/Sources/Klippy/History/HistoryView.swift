import App
import SwiftUI

/// The History window: every copied item in a searchable sidebar, and the
/// selected one shown in full.
struct HistoryView: View {
    static let windowID = "history"

    @ObservedObject var core: Core

    var body: some View {
        NavigationSplitView {
            sidebar
                .navigationSplitViewColumnWidth(min: 220, ideal: 280, max: 420)
        } detail: {
            if let selected = core.view.selected {
                ClipViewer(clip: selected)
                    .toolbar {
                        Button {
                            core.update(.copyItem(selected.id))
                        } label: {
                            Label("Copy", systemImage: "doc.on.doc")
                        }
                        .help("Copy")
                    }
            } else {
                ContentUnavailableView(
                    "No Selection",
                    systemImage: "doc.on.clipboard",
                    description: Text("Select a copied item in the sidebar.")
                )
            }
        }
        .searchable(
            text: Binding(
                get: { core.view.historySearch },
                set: { core.update(.historySearch($0)) }
            ),
            placement: .sidebar
        )
    }

    /// Same cards as the main window; `List` only renders visible rows.
    private var sidebar: some View {
        List(core.view.historyItems, id: \.id) { item in
            ClipRow(
                item: item,
                isSelected: item.id == core.view.selected?.id,
                onTap: { core.update(.selectItem(item.id)) },
                onCopy: { core.update(.copyItem(item.id)) }
            )
            .listRowSeparator(.hidden)
            .listRowBackground(Color.clear)
            .listRowInsets(EdgeInsets(top: 3, leading: 8, bottom: 3, trailing: 8))
        }
        .listStyle(.plain)
        .scrollContentBackground(.hidden)
        .overlay {
            if core.view.historyItems.isEmpty {
                Text(core.view.historySearch.isEmpty ? "Nothing copied yet" : "No matches")
                    .foregroundStyle(.secondary)
            }
        }
    }
}
