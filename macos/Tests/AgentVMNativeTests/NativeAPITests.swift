import Foundation
import Testing
@testable import AgentVMNative

@Suite("NativeAPITests")
struct NativeAPITests {
    @Test(.enabled(if: NativeTestEnvironment.endpoint != nil))
    func aCancelledRealRequestUsesStructuredCancellation() async throws {
        let api = NativeAPI(baseURL: try #require(NativeTestEnvironment.endpoint))
        let request = Task { try await api.request("/api/claude/versions") }
        request.cancel()
        do {
            _ = try await request.value
            Issue.record("A pre-cancelled real URLSession request must not succeed")
        } catch is CancellationError {
        } catch {
            Issue.record("Expected CancellationError rather than a user-visible networking error: \(error)")
        }
    }

    @Test
    func unconfiguredBackupErrorsRequireBadRequestAndAcceptServerWording() {
        #expect(NativeAPIError(status: 400, message: "S3 is not configured: set it up in Settings").unconfiguredBackup)
        #expect(NativeAPIError(status: 400, message: "S3 backups are not configured").unconfiguredBackup)
        #expect(!NativeAPIError(status: 502, message: "S3 is not configured upstream").unconfiguredBackup)
    }

    @Test
    func testPathsCannotReplaceConfiguredServerOrigin() {
        let api = NativeAPI(baseURL: URL(string: "http://127.0.0.1:7791")!)
        for path in ["/api/tasks", "api/tasks", "//example.com/api/tasks", "https://example.com/api/tasks"] {
            let url = api.url(path)
            #expect(url.host == "127.0.0.1")
            #expect(url.port == 7791)
            #expect(url.scheme == "http")
        }
        #expect(api.url("/api/repos/check?path=%2Ftmp%2Fwork").query == "path=%2Ftmp%2Fwork")
    }

    @Test(.enabled(if: NativeTestEnvironment.endpoint != nil))
    func testRealSandboxAPIReadAndErrorContract() async throws {
        let base = try #require(NativeTestEnvironment.endpoint)
        let api = NativeAPI(baseURL: base)
        let status = try await api.request("/api/status")
        #expect(status["host"]["cpus"].integer > 0)
        let settings = try await api.request("/api/settings")
        #expect(settings["settings"]["memory_mb"].integer > 0)
        let tasks = try await api.request("/api/tasks")
        guard case .array = tasks else {
            Issue.record("Task listing must be a JSON array")
            return
        }
        do {
            _ = try await api.request("/api/tasks/not-a-valid-task-id")
            Issue.record("The server's 404 must be surfaced")
        } catch let error as NativeAPIError {
            #expect(error.status == 404)
            #expect(!(error.message.isEmpty))
        }
    }

    @Test(.enabled(if: NativeTestEnvironment.endpoint != nil))
    func testRealSandboxFailedDownloadPreservesAnExistingDestination() async throws {
        let base = try #require(NativeTestEnvironment.endpoint)
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        let destination = folder.appendingPathComponent("snapshot.tar.zst")
        let original = Data("existing archive".utf8)
        try original.write(to: destination)
        do {
            try await NativeAPI(baseURL: base).download("/api/snapshots/not-a-snapshot/export", to: destination)
            Issue.record("A failed HTTP download must not replace a file")
        } catch let error as NativeAPIError { #expect(error.status == 404) }
        #expect(try Data(contentsOf: destination) == original)
    }
}
