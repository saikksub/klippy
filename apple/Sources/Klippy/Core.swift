import App
import AppKit
import Shared
import UserNotifications

@MainActor
final class Core: ObservableObject {
    @Published var view: ViewModel

    private let core: CoreFfi
    private var clipboardWatcher: Task<Void, Never>?
    private var watchActivity: NSObjectProtocol?
    private var lastChangeCount = 0

    init() {
        core = CoreFfi()
        // swiftlint:disable:next force_try
        view = try! .bincodeDeserialize(input: [UInt8](core.view()))
    }

    func update(_ event: Event) {
        // swiftlint:disable:next force_try
        processEffects(core.update(data: Data(try! event.bincodeSerialize())))
    }

    /// Polls the clipboard's change counter (macOS has no change notification)
    /// and reports each change to the core. Only the counter is read here, so
    /// ordinary copies never touch the clipboard's content.
    func watchClipboard() {
        guard clipboardWatcher == nil else { return }
        lastChangeCount = NSPasteboard.general.changeCount
        // Opt out of App Nap: with its windows closed, Klippy would otherwise
        // have this timer throttled and miss the timing of quick copies.
        watchActivity = ProcessInfo.processInfo.beginActivity(
            options: .userInitiatedAllowingIdleSystemSleep,
            reason: "Watching the clipboard for repeated copies"
        )
        clipboardWatcher = Task { [weak self] in
            while !Task.isCancelled {
                self?.pollClipboard()
                // Fast enough to time a triple copy within its 1.5s window.
                try? await Task.sleep(for: .milliseconds(100))
            }
        }
    }

    private func pollClipboard() {
        let changeCount = NSPasteboard.general.changeCount
        guard changeCount != lastChangeCount else { return }
        let count = UInt32(max(changeCount - lastChangeCount, 1))
        lastChangeCount = changeCount
        let atMs = UInt64(ProcessInfo.processInfo.systemUptime * 1000)
        update(.clipboardChanged(count: count, atMs: atMs))
    }

    /// The clipboard's text with every format it's available in, or nil if it
    /// has no text or the source app marked it private (https://nspasteboard.org).
    private static func readClipboard() -> ClipboardContent? {
        guard let item = NSPasteboard.general.pasteboardItems?.first,
              let text = item.string(forType: .string)
        else { return nil }
        let privateTypes: Set<String> = ["org.nspasteboard.ConcealedType", "org.nspasteboard.TransientType"]
        guard privateTypes.isDisjoint(with: item.types.map(\.rawValue)) else { return nil }

        let formats = item.types.compactMap { type in
            item.data(forType: type).map { ClipboardFormat(kind: type.rawValue, data: [UInt8]($0)) }
        }
        return ClipboardContent(text: text, formats: formats)
    }

    private func processEffects(_ effects: Data) {
        // swiftlint:disable:next force_try
        let requests = try! Requests.bincodeDeserialize(input: [UInt8](effects)).value
        for request in requests {
            processEffect(request)
        }
    }

    private func processEffect(_ request: Request) {
        switch request.effect {
        case .render:
            // swiftlint:disable:next force_try
            view = try! .bincodeDeserialize(input: [UInt8](core.view()))
        case .clipboardAccess:
            let access = ClipboardAccess(granted: Self.canReadClipboard())
            // swiftlint:disable:next force_try
            processEffects(core.resolve(id: request.id, data: Data(try! access.bincodeSerialize())))
        case .notificationPermission:
            let id = request.id
            Task {
                let permission = NotificationPermission(granted: await Self.requestNotificationPermission())
                // swiftlint:disable:next force_try
                processEffects(core.resolve(id: id, data: Data(try! permission.bincodeSerialize())))
            }
        case .clipboardRead:
            let snapshot = ClipboardSnapshot(content: Self.readClipboard())
            // swiftlint:disable:next force_try
            processEffects(core.resolve(id: request.id, data: Data(try! snapshot.bincodeSerialize())))
        case let .clipboardWrite(write):
            let item = NSPasteboardItem()
            item.setString(write.content.text, forType: .string)
            for format in write.content.formats {
                item.setData(Data(format.data), forType: NSPasteboard.PasteboardType(format.kind))
            }
            NSPasteboard.general.clearContents()
            NSPasteboard.general.writeObjects([item])
            // Our own write isn't a user copy, so don't count it.
            lastChangeCount = NSPasteboard.general.changeCount
        }
    }

    /// Shows the system's Allow / Don't Allow prompt the first time; after that
    /// returns the stored answer without prompting.
    private static func requestNotificationPermission() async -> Bool {
        // UserNotifications needs a real .app bundle; `swift run` and
        // `swift test` aren't one and would crash.
        guard Bundle.main.bundleURL.pathExtension == "app" else { return false }
        let center = UNUserNotificationCenter.current()
        return (try? await center.requestAuthorization(options: [.alert, .sound, .badge])) ?? false
    }

    /// True only when the app can read the general pasteboard without the
    /// system prompting the user (macOS 15.4+ pasteboard privacy).
    private static func canReadClipboard() -> Bool {
        guard #available(macOS 15.4, *) else { return true }
        return NSPasteboard.general.accessBehavior == .alwaysAllow
    }
}
