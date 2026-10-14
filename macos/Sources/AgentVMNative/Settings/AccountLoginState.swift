import Combine

@MainActor
final class NativeAccountLoginState: ObservableObject {
    @Published private(set) var session: NativeAccountLoginSession?
    @Published private(set) var phase = NativeAccountLoginPhase.pending
    @Published private(set) var retryGeneration = 0
    var polls = 0

    func begin(_ session: NativeAccountLoginSession) {
        self.session = session
        setPhase(.pending)
        polls = 0
    }

    func clear() {
        session = nil
        setPhase(.pending)
        polls = 0
    }

    func setPhase(_ phase: NativeAccountLoginPhase) {
        guard self.phase != phase else { return }
        self.phase = phase
    }

    @discardableResult
    func setPhase(_ phase: NativeAccountLoginPhase, for capturedID: String) -> Bool {
        guard isCurrent(capturedID) else { return false }
        setPhase(phase)
        return true
    }

    func isCurrent(_ capturedID: String) -> Bool { session?.id == capturedID }

    func acceptResponse(_ response: JSON, for capturedID: String) -> Bool {
        setPhase(NativeAccountLoginPolicy.phase(response: response, id: capturedID), for: capturedID)
    }

    func retry() {
        setPhase(.pending)
        polls = 0
        retryGeneration += 1
    }
}
