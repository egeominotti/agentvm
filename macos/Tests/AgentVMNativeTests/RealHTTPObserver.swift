import Foundation
@testable import AgentVMNative

enum RealHTTPObserver {
    static func start(target: URL, folder: URL) async throws -> (Process, URL) {
        let script = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("Support/counting-proxy.ts")
        let ready = folder.appendingPathComponent("observer.json")
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["bun", script.path, target.absoluteString, ready.path]
        // The observer forwards real HTTP and needs no credentials from the test process.
        process.environment = ["PATH": ProcessInfo.processInfo.environment["PATH"] ?? "/usr/bin:/bin"]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
        var started = false
        defer { if !started && process.isRunning { process.terminate(); process.waitUntilExit() } }
        for _ in 0..<200 {
            if let data = try? Data(contentsOf: ready), let state = try? JSONDecoder().decode(JSON.self, from: data),
               state["port"].integer > 0, let url = URL(string: "http://127.0.0.1:\(state["port"].integer)") {
                started = true
                return (process, url)
            }
            if !process.isRunning { break }
            try await Task.sleep(for: .milliseconds(25))
        }
        throw RuntimeError.message("The real HTTP observer could not start.")
    }
}
