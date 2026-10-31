import App
import SwiftUI

/// The History window: saved copies (or this session's failed ones) in a
/// sidebar with tabs at the bottom, and the selected one shown in full.
struct HistoryView: View {
    static let windowID = "history"

    @ObservedObject var core: Core

    private var tab: HistoryTab { core.view.historyTab }

    var body: some View {
        NavigationSplitView {
            sidebar
                .safeAreaInset(edge: .bottom, spacing: 0) { tabBar }
                .navigationSplitViewColumnWidth(min: 220, ideal: 280, max: 420)
        } detail: {
            detail
        }
        .searchable(
            text: Binding(
                get: { core.view.historySearch },
                set: { core.update(.historySearch($0)) }
            ),
            placement: .sidebar,
            prompt: "Search saved copies"
        )
        .onAppear(perform: selectFirstIfNone)
        // Also when items arrive later, the selected one goes, or tabs change.
        .onChange(of: core.view.historyItems) { selectFirstIfNone() }
        .onChange(of: core.view.failedItems) { selectFirstIfNone() }
        .onChange(of: tab) { selectFirstIfNone() }
    }

    /// Opening a tab with nothing selected shows its first item.
    private func selectFirstIfNone() {
        switch tab {
        case .saved:
            guard core.view.selected == nil, let first = core.view.historyItems.first else { return }
            core.update(.selectItem(first.id))
        case .failed:
            guard core.view.selectedFailed == nil, let first = core.view.failedItems.first else { return }
            core.update(.selectFailed(first.id))
        }
    }

    // MARK: Sidebar

    /// Same cards as the main window; `List` only renders visible rows.
    @ViewBuilder private var sidebar: some View {
        switch tab {
        case .saved:
            cardList(core.view.historyItems, selectedID: core.view.selected?.id) { item in
                core.update(.selectItem(item.id))
            } onCopy: { item in
                core.update(.copyItem(item.id))
            }
            .overlay {
                if core.view.historyItems.isEmpty {
                    Text(core.view.historySearch.isEmpty ? "Nothing copied yet" : "No matches")
                        .foregroundStyle(.secondary)
                }
            }
        case .failed:
            cardList(core.view.failedItems, selectedID: core.view.selectedFailed?.id) { item in
                core.update(.selectFailed(item.id))
            } onCopy: { item in
                core.update(.copyFailedText(item.id))
            }
            .overlay {
                if core.view.failedItems.isEmpty {
                    ContentUnavailableView(
                        "No Failed Copies",
                        systemImage: "checkmark.shield",
                        description: Text("Copies with malicious content show up here until Klippy quits.")
                    )
                }
            }
        }
    }

    private func cardList(
        _ items: [ClipItem],
        selectedID: UInt64?,
        onTap: @escaping (ClipItem) -> Void,
        onCopy: @escaping (ClipItem) -> Void
    ) -> some View {
        List(items, id: \.id) { item in
            ClipRow(
                item: item,
                isSelected: item.id == selectedID,
                onTap: { onTap(item) },
                onCopy: { onCopy(item) }
            )
            .listRowSeparator(.hidden)
            .listRowBackground(Color.clear)
            .listRowInsets(EdgeInsets(top: 3, leading: 8, bottom: 3, trailing: 8))
        }
        .listStyle(.plain)
        .scrollContentBackground(.hidden)
    }

    private var tabBar: some View {
        VStack(spacing: 0) {
            Divider()
            HStack(spacing: 4) {
                SidebarTab(title: "Saved", systemImage: "doc.on.clipboard", isSelected: tab == .saved) {
                    core.update(.setHistoryTab(.saved))
                }
                SidebarTab(
                    title: "Failed",
                    systemImage: "exclamationmark.shield",
                    badge: core.view.failedItems.count,
                    isSelected: tab == .failed
                ) {
                    core.update(.setHistoryTab(.failed))
                }
            }
            .padding(8)
        }
        .background(.bar)
    }

    // MARK: Detail

    @ViewBuilder private var detail: some View {
        switch tab {
        case .saved:
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
        case .failed:
            if let failed = core.view.selectedFailed {
                FailedCopyView(failed: failed)
                    .toolbar {
                        Button {
                            core.update(.copyFailedText(failed.id))
                        } label: {
                            Label("Copy Text", systemImage: "doc.on.doc")
                        }
                        .help("Copy only the plain text")
                        Button {
                            core.update(.dismissFailed(failed.id))
                        } label: {
                            Label("Dismiss", systemImage: "xmark.circle")
                        }
                        .help("Remove from this list")
                        Button("Dismiss All") { core.update(.dismissAllFailed) }
                    }
            } else {
                ContentUnavailableView(
                    "No Failed Copies",
                    systemImage: "checkmark.shield",
                    description: Text("Nothing has been blocked this session.")
                )
            }
        }
    }
}

/// One tab at the bottom of the sidebar, with an optional count badge.
private struct SidebarTab: View {
    let title: String
    let systemImage: String
    var badge = 0
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Image(systemName: systemImage)
                Text(title)
                if badge > 0 {
                    Text("\(badge)")
                        .font(.caption2.weight(.bold))
                        .monospacedDigit()
                        .padding(.horizontal, 6)
                        .padding(.vertical, 1)
                        .background(Capsule().fill(Color.red))
                        .foregroundStyle(.white)
                }
            }
            .font(.callout.weight(isSelected ? .semibold : .regular))
            .frame(maxWidth: .infinity)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(isSelected ? Color.primary.opacity(0.1) : .clear)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .foregroundStyle(isSelected ? .primary : .secondary)
    }
}

/// Why a copy wasn't saved, and its plain text. The HTML itself is never
/// kept or shown.
private struct FailedCopyView: View {
    let failed: FailedCopy

    private var failedAt: String {
        Date(timeIntervalSince1970: TimeInterval(failed.failedAtMs) / 1000)
            .formatted(date: .abbreviated, time: .shortened)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack(alignment: .top, spacing: 12) {
                    Image(systemName: "exclamationmark.shield.fill")
                        .font(.system(size: 30))
                        .foregroundStyle(.red)
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Not saved: malicious content")
                            .font(.title3.weight(.semibold))
                        Text("Blocked \(failedAt). Kept only until Klippy quits.")
                            .foregroundStyle(.secondary)
                    }
                }

                VStack(alignment: .leading, spacing: 8) {
                    Text("What Klippy found")
                        .font(.headline)
                    ForEach(failed.reasons, id: \.self) { reason in
                        Label(reason, systemImage: "xmark.octagon.fill")
                            .symbolRenderingMode(.multicolor)
                    }
                }

                VStack(alignment: .leading, spacing: 8) {
                    Text("Plain text")
                        .font(.headline)
                    Text(failed.text)
                        .font(.body.monospaced())
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(12)
                        .background(
                            RoundedRectangle(cornerRadius: 10, style: .continuous)
                                .fill(Color.primary.opacity(0.05))
                        )
                }
            }
            .padding(28)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}
