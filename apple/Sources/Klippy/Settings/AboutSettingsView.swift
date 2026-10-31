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
            // Required by the icon's CC BY 4.0 licence.
            Section("Logo") {
                LabeledContent("Icon", value: "solar:paperclip-rounded-2-linear")
                LabeledContent("From") {
                    Link(
                        "Solar Icons Set by 480 Design",
                        destination: URL(string: "https://www.figma.com/community/file/1166831539721848736/solar-icons-set")!
                    )
                }
                LabeledContent("Licence") {
                    Link("CC BY 4.0", destination: URL(string: "https://creativecommons.org/licenses/by/4.0/")!)
                }
            }
        }
        .formStyle(.grouped)
    }
}
