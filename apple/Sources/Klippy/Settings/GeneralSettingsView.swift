import App
import ServiceManagement
import SwiftUI

/// Grouped rows with pop-up menus, like System Settings.
struct GeneralSettingsView: View {
    @ObservedObject var core: Core
    @ObservedObject var launchAtLogin: LaunchAtLogin
    @Environment(\.openWindow) private var openWindow

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
                Picker("Delete items older than", selection: retentionChoice) {
                    ForEach(Self.retentionPresets, id: \.self) { preset in
                        Text(preset.label).tag(RetentionChoice.preset(preset))
                    }
                    Divider()
                    Text("Custom…").tag(RetentionChoice.custom)
                }
                if showsCustomRetention {
                    LabeledContent("Custom") {
                        HStack(spacing: 8) {
                            TextField("Amount", value: retentionAmount, format: .number)
                                .labelsHidden()
                                .textFieldStyle(.roundedBorder)
                                .multilineTextAlignment(.center)
                                .monospacedDigit()
                                .frame(width: 56)
                            Picker("Unit", selection: retentionUnit) {
                                Text("minutes").tag(RetentionUnit.minutes)
                                Text("hours").tag(RetentionUnit.hours)
                                Text("days").tag(RetentionUnit.days)
                            }
                            .labelsHidden()
                            .fixedSize()
                        }
                        .help("From 1 minute up to 365 days")
                    }
                }
            } footer: {
                Text(core.view.retentionHint)
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }
            .disabled(!core.view.unlocked)

            Section {
                LabeledContent("Saved copies") {
                    Button("Clear History…", role: .destructive) {
                        openWindow(id: ClearHistoryView.windowID)
                    }
                }
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

    private struct RetentionPreset: Hashable {
        let amount: UInt32
        let unit: RetentionUnit
        let label: String
    }

    private enum RetentionChoice: Hashable {
        case preset(RetentionPreset)
        case custom
    }

    private static let retentionPresets = [
        RetentionPreset(amount: 1, unit: .hours, label: "1 hour"),
        RetentionPreset(amount: 1, unit: .days, label: "1 day"),
        RetentionPreset(amount: 7, unit: .days, label: "7 days"),
        RetentionPreset(amount: 30, unit: .days, label: "30 days"),
        RetentionPreset(amount: 90, unit: .days, label: "90 days"),
        RetentionPreset(amount: 365, unit: .days, label: "1 year"),
    ]

    /// Chosen "Custom…" this time, so typing a preset's value doesn't snap
    /// the menu back to that preset and hide the field.
    @State private var prefersCustomRetention = false

    private var currentPreset: RetentionPreset? {
        Self.retentionPresets.first {
            $0.amount == core.view.retentionAmount && $0.unit == core.view.retentionUnit
        }
    }

    private var showsCustomRetention: Bool {
        prefersCustomRetention || currentPreset == nil
    }

    private var retentionChoice: Binding<RetentionChoice> {
        Binding(
            get: { showsCustomRetention ? .custom : currentPreset.map(RetentionChoice.preset) ?? .custom },
            set: { choice in
                switch choice {
                case let .preset(preset):
                    prefersCustomRetention = false
                    core.update(.setRetention(amount: preset.amount, unit: preset.unit))
                case .custom:
                    prefersCustomRetention = true
                }
            }
        )
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
