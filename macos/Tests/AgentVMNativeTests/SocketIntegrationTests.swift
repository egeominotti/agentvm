import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native real terminal transport", .serialized)
struct SocketIntegrationTests {
    @Test(.enabled(if: NativeTestEnvironment.canChangeSnapshots &&
                          ProcessInfo.processInfo.environment["AGENTVM_NATIVE_TERMINAL_TASK_ID"] != nil &&
                          ProcessInfo.processInfo.environment["AGENTVM_NATIVE_TERMINAL_SHARE_PATH"] != nil))
    @MainActor
    func delayedHandshakeAndReconnectKeepOrderedInput() async throws {
        let environment = ProcessInfo.processInfo.environment
        let api = NativeAPI(baseURL: try #require(NativeTestEnvironment.snapshotEndpoint))
        try #require(api.baseURL.port != 7777)
        let id = try #require(environment["AGENTVM_NATIVE_TERMINAL_TASK_ID"])
        let share = URL(fileURLWithPath: try #require(environment["AGENTVM_NATIVE_TERMINAL_SHARE_PATH"]))
        let task = try await api.request("/api/tasks/\(id)")
        try #require(task.state == "running")
        try #require(FileManager.default.fileExists(atPath: share.path))
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("native-socket-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        let (proxy, proxyURL) = try await startProxy(target: api.baseURL, folder: folder)
        defer { if proxy.isRunning { proxy.terminate(); proxy.waitUntilExit() } }
        let filename = "native-fifo-\(UUID().uuidString)"
        let output = share.appendingPathComponent(filename)
        defer { try? FileManager.default.removeItem(at: output) }
        var opens = 0
        var disconnected = false
        let transport = NativeTerminalSocket(onStatus: { status in
            if status { opens += 1 } else if opens > 0 { disconnected = true }
        }, onOutput: { _ in })
        defer { transport.close() }
        var parts = URLComponents(url: proxyURL.appendingPathComponent("api/tasks/\(id)/pty"), resolvingAgainstBaseURL: false)!
        parts.scheme = "ws"
        parts.queryItems = [URLQueryItem(name: "session", value: "shell-8")]
        var request = URLRequest(url: try #require(parts.url))
        request.setValue(api.baseURL.absoluteString, forHTTPHeaderField: "Origin")
        transport.resize(cols: 100, rows: 30)
        transport.connect(request)
        func append(_ value: String) -> Bool {
            transport.input(Data("printf '%s\\n' '\(value)' >> '/mnt/job/\(filename)'\r".utf8))
        }
        var failure: Error?
        do {
            #expect(append("first"))
            #expect(append("second"))
            try await Task.sleep(for: .milliseconds(150))
            #expect(opens == 0)
            #expect(!FileManager.default.fileExists(atPath: output.path))
            try await wait { (try? String(contentsOf: output, encoding: .utf8)) == "first\nsecond\n" }
            #expect(opens == 1)
            try Data().write(to: folder.appendingPathComponent("cut"))
            try await wait { disconnected }
            #expect(append("third"))
            #expect(append("fourth"))
            try await wait { (try? String(contentsOf: output, encoding: .utf8)) == "first\nsecond\nthird\nfourth\n" }
            #expect(opens == 2)
            #expect(try String(contentsOf: output, encoding: .utf8) == "first\nsecond\nthird\nfourth\n")
        } catch { failure = error }
        transport.close()
        if opens > 0 { _ = try await api.request("/api/tasks/\(id)/pty/shell-8", method: "DELETE") }
        if let failure { throw failure }
    }

    @MainActor
    private func wait(_ ready: () -> Bool) async throws {
        for _ in 0..<400 {
            if ready() { return }
            try await Task.sleep(for: .milliseconds(25))
        }
        try #require(ready(), "The real terminal did not execute its queued input within ten seconds.")
    }

    private func startProxy(target: URL, folder: URL) async throws -> (Process, URL) {
        let script = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("Support/delayed-socket-proxy.py")
        let ready = folder.appendingPathComponent("ready.json")
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/python3")
        process.arguments = [script.path, String(try #require(target.port)), ready.path, folder.appendingPathComponent("cut").path]
        process.environment = ["PATH": "/usr/bin:/bin"]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
        var started = false
        defer { if !started && process.isRunning { process.terminate(); process.waitUntilExit() } }
        for _ in 0..<200 {
            if let data = try? Data(contentsOf: ready), let value = try? JSONDecoder().decode(JSON.self, from: data),
               let url = URL(string: "http://127.0.0.1:\(value["port"].integer)"), value["port"].integer > 0 {
                started = true
                return (process, url)
            }
            if !process.isRunning { break }
            try await Task.sleep(for: .milliseconds(25))
        }
        throw RuntimeError.message("The real WebSocket delay proxy could not start.")
    }
}
