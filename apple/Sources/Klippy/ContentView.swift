import App
import SwiftUI

struct ContentView: View {
    @ObservedObject var core: Core

    var body: some View {
        VStack(spacing: 16) {
            Text(core.view.count)
                .font(.title)
            HStack {
                Button("Reset", role: .destructive) { core.update(.reset) }
                Button("−") { core.update(.decrement) }
                Button("+") { core.update(.increment) }
                    .keyboardShortcut(.defaultAction)
            }
            .controlSize(.large)
        }
        .padding()
    }
}
