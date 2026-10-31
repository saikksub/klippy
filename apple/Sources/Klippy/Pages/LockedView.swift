import App
import SwiftUI

/// Shown when Touch ID was cancelled or failed; the history stays encrypted.
struct LockedView: View {
    @ObservedObject var core: Core

    var body: some View {
        VStack(spacing: 14) {
            Image(systemName: "lock.fill")
                .font(.system(size: 34))
                .foregroundStyle(.secondary)
            Text("Klippy is locked")
                .font(.title3.weight(.semibold))
            if !core.view.unlockError.isEmpty {
                Text(core.view.unlockError)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
            }
            Button {
                core.update(.start)
            } label: {
                Label("Unlock with Touch ID", systemImage: "touchid")
            }
            .controlSize(.large)
            .keyboardShortcut(.defaultAction)
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
