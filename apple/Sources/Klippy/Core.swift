import App
import Foundation
import Shared

@MainActor
final class Core: ObservableObject {
    @Published var view: ViewModel

    private let core: CoreFfi

    init() {
        core = CoreFfi()
        // swiftlint:disable:next force_try
        view = try! .bincodeDeserialize(input: [UInt8](core.view()))
    }

    func update(_ event: Event) {
        // swiftlint:disable:next force_try
        let effects = [UInt8](core.update(data: Data(try! event.bincodeSerialize())))
        // swiftlint:disable:next force_try
        let requests = try! Requests.bincodeDeserialize(input: effects).value
        for request in requests {
            processEffect(request)
        }
    }

    private func processEffect(_ request: Request) {
        switch request.effect {
        case .render:
            // swiftlint:disable:next force_try
            view = try! .bincodeDeserialize(input: [UInt8](core.view()))
        }
    }
}
