import SwiftUI

/// App identity over grouped rows, like System Settings' About page.
struct AboutSettingsView: View {
    private let info = Bundle.main.infoDictionary

    var body: some View {
        Form {
            Section {
                VStack(spacing: 6) {
                    Image(nsImage: NSApp.applicationIconImage)
                        .resizable()
                        .frame(width: 64, height: 64)
                    Text("Klippy")
                        .font(.title2.weight(.semibold))
                }
                .frame(maxWidth: .infinity)
            }
            Section {
                LabeledContent("Version", value: info?["CFBundleShortVersionString"] as? String ?? "–")
                LabeledContent("Build", value: info?["CFBundleVersion"] as? String ?? "–")
            }
            Section {
                LabeledContent("Made by", value: "Xplorebits Technologies")
                LabeledContent("Website") {
                    Link("xplorebits.com", destination: URL(string: "https://xplorebits.com")!)
                }
            }
        }
        .formStyle(.grouped)
    }
}
