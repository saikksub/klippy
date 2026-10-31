import App
import ServiceManagement
import SwiftUI

/// Grouped rows with pop-up menus, like System Settings.
struct GeneralSettingsView: View {
    @ObservedObject var core: Core
    @ObservedObject var launchAtLogin: LaunchAtLogin

    var body: some View {
        Form {
            Section {
                Toggle("Launch at login", isOn: Binding(
                    get: { launchAtLogin.isEnabled },
                    set: { launchAtLogin.setEnabled($0) }
                ))
                if launchAtLogin.needsApproval {
                    LabeledContent("Turned off in System Settings") {
                        Button("Open Login Items…") {
                            SMAppService.openSystemSettingsLoginItems()
                        }
                    }
                }
                if let error = launchAtLogin.error {
                    Text(error)
                        .font(.callout)
                        .foregroundStyle(.red)
                }
            }

            Section {
                Picker("Copy trigger", selection: trigger) {
                    ForEach([UInt32](1...4), id: \.self) { count in
                        Text(count == 1 ? "1 time" : "\(count) times").tag(count)
                    }
                }
            } footer: {
                Text(core.view.copyTriggerHint)
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }

            Section {
                LabeledContent("Notifications") {
                    HStack {
                        Text(notificationStatus)
                            .foregroundStyle(.secondary)
                        if core.view.notificationsAllowed == false {
                            Button("Open System Settings…") {
                                NSWorkspace.shared.open(Self.notificationSettingsURL)
                            }
                        }
                    }
                }
            }
        }
        .formStyle(.grouped)
        .onAppear {
            core.update(.refreshNotificationPermission)
            launchAtLogin.refresh()
        }
    }

    private static let notificationSettingsURL =
        URL(string: "x-apple.systempreferences:com.apple.Notifications-Settings.extension")!

    private var notificationStatus: String {
        switch core.view.notificationsAllowed {
        case true?: "Allowed"
        case false?: "Off"
        case nil: "Not asked yet"
        }
    }

    private var trigger: Binding<UInt32> {
        Binding(
            get: { core.view.copyTrigger },
            set: { core.update(.setCopyTrigger($0)) }
        )
    }
}
