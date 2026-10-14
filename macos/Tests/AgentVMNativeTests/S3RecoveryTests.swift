import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native real S3 recovery", .serialized)
struct S3RecoveryTests {
    @MainActor
    @Test(.enabled(if: NativeTestEnvironment.s3RecoveryEndpoint != nil))
    func successfulSavedConnectionTestRefreshesUnavailableBackupListing() async throws {
        let target = try #require(NativeTestEnvironment.s3RecoveryEndpoint)
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("native-s3-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        let (observer, url) = try await RealHTTPObserver.start(target: target, folder: folder)
        defer { if observer.isRunning { observer.terminate(); observer.waitUntilExit() } }
        let store = NativeStore(baseURL: url)
        await store.refresh()
        try #require(store.connected)
        let endpoint = try #require(URL(string: store.settings["s3"]["endpoint"].text))
        try #require(endpoint.scheme == "http" && endpoint.host == "127.0.0.1")
        let port = try #require(endpoint.port)
        try #require(port != 7777 && port != 9100 && port != target.port)
        await store.refreshSnapshots()
        try #require(store.backupsError != nil)
        #expect(store.error == nil)
        let before = try await store.api.request("/__native_counts")
        #expect(before["GET /api/backups"].integer == 1)
        let forwarder = try await startForwarder(port: port, folder: folder)
        defer { if forwarder.isRunning { forwarder.terminate(); forwarder.waitUntilExit() } }
        let succeeded = await store.testS3Connection()
        #expect(succeeded)
        #expect(store.error == nil)
        #expect(store.backupsError == nil)
        #expect(!store.backupsLoading)
        let counts = try await store.api.request("/__native_counts")
        #expect(counts["POST /api/settings/s3/test"].integer == 1)
        #expect(counts["GET /api/backups"].integer == 2)
        let actual = try await store.api.request("/api/backups").items
        #expect(store.backups == actual)
    }

    private func startForwarder(port: Int, folder: URL) async throws -> Process {
        let script = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("Support/raw-tcp-proxy.py")
        let ready = folder.appendingPathComponent("forwarder.json")
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/python3")
        process.arguments = [script.path, String(port), "9100", ready.path]
        process.environment = ["PATH": "/usr/bin:/bin"]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
        var started = false
        defer { if !started && process.isRunning { process.terminate(); process.waitUntilExit() } }
        for _ in 0..<200 {
            if FileManager.default.fileExists(atPath: ready.path) { started = true; return process }
            if !process.isRunning { break }
            try await Task.sleep(for: .milliseconds(25))
        }
        throw RuntimeError.message("The real S3 TCP forwarder could not start.")
    }
}
