import App
import Testing
@testable import Klippy

@MainActor
struct CoreTests {
    @Test func roundTripsEventsThroughRustCore() {
        let core = Core()
        #expect(core.view.page == .splash)
        #expect(core.view.count == "Count is: 0")

        core.update(.increment)
        core.update(.increment)
        #expect(core.view.count == "Count is: 2")

        core.update(.reset)
        core.update(.decrement)
        #expect(core.view.count == "Count is: -1")

        core.update(.navigate(.home))
        #expect(core.view.page == .home)
    }

    @Test func startChecksClipboardAccessThenLeavesSplash() {
        let core = Core()
        #expect(core.view.clipboardAccess == "")

        core.update(.start)
        #expect(core.view.page == (core.view.clipboardAccess == "true" ? .home : .setup))
        #expect(["true", "false"].contains(core.view.clipboardAccess))
    }

    @Test func searchFiltersCopiedText() {
        let core = Core()
        core.update(.copied(ClipboardSnapshot(content: ClipboardContent(text: "Hello world", formats: []))))
        core.update(.copied(ClipboardSnapshot(content: ClipboardContent(text: "goodbye", formats: []))))
        #expect(core.view.items.map(\.preview) == ["goodbye", "Hello world"])

        core.update(.search("hello"))
        #expect(core.view.items.map(\.preview) == ["Hello world"])
    }

    @Test func copyTriggerDefaultsToThreeAndIsClamped() {
        let core = Core()
        #expect(core.view.copyTrigger == 3)
        core.update(.setCopyTrigger(4))
        #expect(core.view.copyTrigger == 4)
        core.update(.setCopyTrigger(99))
        #expect(core.view.copyTrigger == 4)
    }
}
