// A VM's lifecycle and JSON Lines events on stdout.
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
    private let ptySocket: URL?
    private var bridge: VsockBridge?
    private let startedAt = Date()
    private var signalSources: [DispatchSourceSignal] = []

    init(configuration: VZVirtualMachineConfiguration, ptySocket: URL?) {
        vm = VZVirtualMachine(configuration: configuration)
        self.ptySocket = ptySocket
        super.init()
        vm.delegate = self
    }

    func start() {
        installStopSignals()
        vm.start { [self] result in
            switch result {
            case .success:
                startBridge()
                Events.emit("started")
            case .failure(let e): Events.fail("start failed: \(e.localizedDescription)")
            }
        }
    }

    private func startBridge() {
        guard let path = ptySocket, let device = vm.socketDevices.first as? VZVirtioSocketDevice else { return }
        let bridge = VsockBridge(path: path.path, port: VsockBridge.ptyPort, device: device)
        do { try bridge.start() } catch { Events.fail("terminal socket: \(error)") }
        self.bridge = bridge
    }

    func guestDidStop(_ vm: VZVirtualMachine) {
        finish(ExitCode.clean)
    }

    func virtualMachine(_ vm: VZVirtualMachine, didStopWithError error: Error) {
        Events.fail("the VM stopped with an error: \(error.localizedDescription)")
    }

    /// SIGTERM/SIGINT: forced stop (the disk is disposable).
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
        Events.emit("stopped", ["seconds": (Date().timeIntervalSince(startedAt) * 100).rounded() / 100, "code": Int(code)])
        exit(code)
    }
}
