// Ciclo di vita di una VM ed eventi JSON Lines su stdout.
import Foundation
import Virtualization

enum ExitCode {
    static let clean: Int32 = 0
    static let error: Int32 = 1
    static let stopped: Int32 = 130
}

enum Events {
    static func emit(_ event: String, _ fields: [String: Any] = [:]) {
        var obj = fields
        obj["event"] = event
        guard var data = try? JSONSerialization.data(withJSONObject: obj, options: [.sortedKeys]) else { return }
        data.append(0x0A)
        FileHandle.standardOutput.write(data)
    }

    static func fail(_ message: String) -> Never {
        emit("error", ["message": message])
        exit(ExitCode.error)
    }
}

final class Runner: NSObject, VZVirtualMachineDelegate {
    private let vm: VZVirtualMachine
    private let startedAt = Date()
    private var signalSources: [DispatchSourceSignal] = []

    init(configuration: VZVirtualMachineConfiguration) {
        vm = VZVirtualMachine(configuration: configuration)
        super.init()
        vm.delegate = self
    }

    func start() {
        installStopSignals()
        vm.start { result in
            switch result {
            case .success: Events.emit("started")
            case .failure(let e): Events.fail("avvio fallito: \(e.localizedDescription)")
            }
        }
    }

    func guestDidStop(_ vm: VZVirtualMachine) {
        finish(ExitCode.clean)
    }

    func virtualMachine(_ vm: VZVirtualMachine, didStopWithError error: Error) {
        Events.fail("la VM si è fermata con errore: \(error.localizedDescription)")
    }

    /// SIGTERM/SIGINT: stop forzato (il disco è usa-e-getta).
    private func installStopSignals() {
        for sig in [SIGTERM, SIGINT] {
            signal(sig, SIG_IGN)
            let src = DispatchSource.makeSignalSource(signal: sig, queue: .main)
            src.setEventHandler { [weak self] in self?.forceStop() }
            src.resume()
            signalSources.append(src)
        }
    }

    private func forceStop() {
        guard vm.canStop else { finish(ExitCode.stopped) }
        vm.stop { [weak self] _ in self?.finish(ExitCode.stopped) }
    }

    private func finish(_ code: Int32) -> Never {
        Events.emit("stopped", ["seconds": (Date().timeIntervalSince(startedAt) * 100).rounded() / 100])
        exit(code)
    }
}
