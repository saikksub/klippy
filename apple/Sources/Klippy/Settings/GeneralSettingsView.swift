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
            // Saved in the encrypted history, so only editable once unlocked.
            .disabled(!core.view.unlocked)

            Section {
                LabeledContent("Delete items older than") {
                    HStack(spacing: 6) {
                        TextField("Amount", value: retentionAmount, format: .number)
                            .labelsHidden()
                            .multilineTextAlignment(.trailing)
                            .frame(width: 64)
                        Stepper("Amount", value: retentionAmount, in: 1...Int(core.view.retentionMaxAmount))
                            .labelsHidden()
                        Picker("Unit", selection: retentionUnit) {
                            Text("Minutes").tag(RetentionUnit.minutes)
                            Text("Hours").tag(RetentionUnit.hours)
                            Text("Days").tag(RetentionUnit.days)
                        }
                        .labelsHidden()
                        .fixedSize()
                    }
                }
            } footer: {
                Text(core.view.retentionHint + " Up to 365 days.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }
            .disabled(!core.view.unlocked)

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

    /// The core limits it to 1 minute … 365 days, so out-of-range typing snaps back.
    private var retentionAmount: Binding<Int> {
        Binding(
            get: { Int(core.view.retentionAmount) },
            set: { core.update(.setRetention(amount: UInt32(clamping: $0), unit: core.view.retentionUnit)) }
        )
    }

    private var retentionUnit: Binding<RetentionUnit> {
        Binding(
            get: { core.view.retentionUnit },
            set: { core.update(.setRetention(amount: core.view.retentionAmount, unit: $0)) }
        )
    }

    private var trigger: Binding<UInt32> {
        Binding(
            get: { core.view.copyTrigger },
            set: { core.update(.setCopyTrigger($0)) }
        )
    }
}
