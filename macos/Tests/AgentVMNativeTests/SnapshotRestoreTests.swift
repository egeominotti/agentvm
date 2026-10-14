import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native snapshot restore submission")
struct SnapshotRestoreTests {
    @MainActor
    @Test(.enabled(if: NativeTestEnvironment.canChangeSnapshots))
    func overlappingRestoreActionsIssueOnlyOneRealRequest() async throws {
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        let (process, url) = try await RealHTTPObserver.start(target: #require(NativeTestEnvironment.snapshotEndpoint), folder: folder)
        defer { process.terminate(); process.waitUntilExit() }
        let store = NativeStore(baseURL: url)
        // A real missing snapshot exercises concurrent submissions without creating VMs.
        async let first = store.restoreSnapshot("native-missing-restore")
        async let duplicate = store.restoreSnapshot("native-missing-restore")
        let results = await (first, duplicate)
        #expect(results.0 == nil && results.1 == nil)
        let counts = try await store.api.request("/__native_counts")
        #expect(counts["POST /api/snapshots/native-missing-restore/restore"].integer == 1)
        #expect(!store.busy.contains("/api/snapshots/native-missing-restore/restore"))
        #expect(store.error != nil)
    }
}
