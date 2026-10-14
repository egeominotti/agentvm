import AppKit
import Foundation

@MainActor
final class BackendController: ObservableObject {
    @Published var store: NativeStore?
    @Published var error: String?
    @Published var starting = false
    private var process: Process?

    func connect() async {
        guard !starting else { return }
        starting = true
        error = nil
        defer { starting = false }
        do {
            let environment = ProcessInfo.processInfo.environment
            let config = try RuntimeConfiguration(environment: environment)
            let api = NativeAPI(baseURL: config.endpoint)
            if let status = try? await api.request("/api/status"), RuntimeConfiguration.isAgentVM(status) {
                await ready(config.endpoint)
                return
            }
            // An existing engine is reused; this app never restarts it or stops its machines.
            if process?.isRunning != true { try launch(config, environment: environment) }
            for _ in 0..<100 {
                if let status = try? await api.request("/api/status"), RuntimeConfiguration.isAgentVM(status) {
                    await ready(config.endpoint)
                    return
                }
                if process?.isRunning == false {
                    throw RuntimeError.message("The engine could not start. Open its log in the AgentVM data folder.")
                }
                try await Task.sleep(for: .milliseconds(200))
            }
            throw RuntimeError.message("The engine did not answer. Check that the local port is available.")
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func ready(_ endpoint: URL) async {
        let store = NativeStore(baseURL: endpoint)
        self.store = store
        await store.refresh()
    }

    private func launch(_ config: RuntimeConfiguration, environment: [String: String]) throws {
        let runtime = environment["AGENTVM_NATIVE_RUNTIME"].map { URL(fileURLWithPath: $0) }
            ?? Bundle.main.resourceURL?.appendingPathComponent("runtime")
            ?? URL(fileURLWithPath: FileManager.default.currentDirectoryPath).deletingLastPathComponent()
        let server = runtime.appendingPathComponent("bin/agentvm-server")
        guard FileManager.default.isExecutableFile(atPath: server.path) else {
            throw RuntimeError.message("The bundled engine is missing. Build the app with macos/build.sh.")
        }
        let logs = config.home.appendingPathComponent("logs")
        try FileManager.default.createDirectory(at: logs, withIntermediateDirectories: true)
        let log = logs.appendingPathComponent("native-server.log")
        if !FileManager.default.fileExists(atPath: log.path) { FileManager.default.createFile(atPath: log.path, contents: nil) }
        let output = try FileHandle(forWritingTo: log)
        try output.seekToEnd()
        defer { try? output.close() }
        let process = Process()
        process.executableURL = server
        process.currentDirectoryURL = runtime
        var childEnvironment = environment
        childEnvironment["AGENTVM_HOME"] = config.home.path
        childEnvironment["AGENTVM_PORT"] = String(config.endpoint.port!)
        childEnvironment["AGENTVM_VM_HELPER"] = environment["AGENTVM_VM_HELPER"]
            ?? runtime.appendingPathComponent("bin/agentvm-vm").path
        process.environment = childEnvironment
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = output
        process.standardError = output
        try process.run()
        self.process = process
        // The engine outlives this window and the app, like the existing command-line engine.
    }

    func showLogs() {
        let home = (try? RuntimeConfiguration(environment: ProcessInfo.processInfo.environment))?.home
        if let home { NSWorkspace.shared.open(home.appendingPathComponent("logs")) }
    }
}
