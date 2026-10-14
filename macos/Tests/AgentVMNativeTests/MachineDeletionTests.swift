import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native machine deletion batches")
struct MachineDeletionTests {
    @MainActor
    @Test(.enabled(if: NativeTestEnvironment.canChangeSnapshots))
    func aRealFailedDeletionPreservesSelectionAndStopsBeforeTheNextRequest() async throws {
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        let (process, url) = try await RealHTTPObserver.start(target: #require(NativeTestEnvironment.snapshotEndpoint), folder: folder)
        defer { process.terminate(); process.waitUntilExit() }
        let store = NativeStore(baseURL: url)
        let completed = await store.performMachineDeletion(ids: ["not-a-task", "another-invalid-task"])
        #expect(completed.isEmpty)
        #expect(store.error != nil)
        let counts = try await store.api.request("/__native_counts")
        #expect(counts["DELETE /api/tasks/not-a-task"].integer == 1)
        #expect(counts["DELETE /api/tasks/another-invalid-task"].integer == 0)
        for path in ["/api/tasks", "/api/status", "/api/settings"] { #expect(counts["GET \(path)"].integer == 1) }
        #expect(counts["GET /api/snapshots"].integer == 0)
        #expect(counts["GET /api/backups"].integer == 0)
    }
}
