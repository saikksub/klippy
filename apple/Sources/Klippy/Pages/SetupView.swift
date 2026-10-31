import SwiftUI

struct SetupView: View {
    var body: some View {
        VStack(spacing: 12) {
            Text("Setup")
                .font(.title2.weight(.semibold))
            Text("Klippy can't read copied text yet.")
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
