import App
import SwiftUI

/// Asks the user to type "confirm" before deleting every saved copy.
struct ClearHistoryView: View {
    static let windowID = "clear-history"
    private static let confirmation = "confirm"

    @ObservedObject var core: Core
    @Environment(\.dismiss) private var dismiss
    @State private var typed = ""

    private var isConfirmed: Bool {
        typed.trimmingCharacters(in: .whitespaces) == Self.confirmation
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(alignment: .top, spacing: 14) {
                Image(systemName: "trash.circle.fill")
                    .font(.system(size: 36))
                    .foregroundStyle(.white, .red)
                VStack(alignment: .leading, spacing: 4) {
                    Text("Clear all history?")
                        .font(.headline)
                    Text("Every saved copy is permanently deleted. This can't be undone.")
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }

            VStack(alignment: .leading, spacing: 6) {
                Text("Type **\(Self.confirmation)** to continue")
                    .font(.callout)
                TextField(Self.confirmation, text: $typed)
                    .textFieldStyle(.roundedBorder)
                    .onSubmit(deleteIfConfirmed)
            }

            HStack {
                Spacer()
                Button("Cancel", role: .cancel) { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Button("Delete All", role: .destructive, action: deleteIfConfirmed)
                    .buttonStyle(.borderedProminent)
                    .tint(.red)
                    .keyboardShortcut(.defaultAction)
                    .disabled(!isConfirmed || !core.view.unlocked)
            }
        }
        .padding(20)
        .frame(width: 380)
        .onAppear { typed = "" }
    }

    private func deleteIfConfirmed() {
        guard isConfirmed, core.view.unlocked else { return }
        core.update(.clearHistory)
        dismiss()
    }
}

/// "Clear History…" in the app menu.
struct ClearHistoryMenuItem: View {
    @ObservedObject var core: Core
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Button("Clear History…") {
            openWindow(id: ClearHistoryView.windowID)
            NSApp.activate(ignoringOtherApps: true)
        }
        .disabled(!core.view.unlocked)
    }
}
