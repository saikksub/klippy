import App
import Testing
@testable import Klippy

@MainActor
struct CoreTests {
    @Test func roundTripsEventsThroughRustCore() {
        let core = Core()
        #expect(core.view.count == "Count is: 0")

        core.update(.increment)
        core.update(.increment)
        #expect(core.view.count == "Count is: 2")

        core.update(.reset)
        core.update(.decrement)
        #expect(core.view.count == "Count is: -1")
    }
}
