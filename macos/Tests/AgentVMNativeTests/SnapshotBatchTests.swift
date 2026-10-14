import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native snapshot batch requests")
struct SnapshotBatchTests {
    private static var fixture: String? { ProcessInfo.processInfo.environment["AGENTVM_NATIVE_SNAPSHOT_ID"] }

    @MainActor
    @Test(.enabled(if: NativeTestEnvironment.canChangeSnapshots))
    func anEmptyBatchMakesNoRequests() async throws {
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        let (process, url) = try await RealHTTPObserver.start(target: #require(NativeTestEnvironment.snapshotEndpoint), folder: folder)
        defer { process.terminate(); process.waitUntilExit() }
        let store = NativeStore(baseURL: url)
        let completed = await store.performSnapshotBatch(ids: [], backup: false)
        #expect(completed.isEmpty)
        let counts = try await store.api.request("/__native_counts")
        #expect(counts.fields.isEmpty)
    }

    @MainActor
    @Test(.enabled(if: NativeTestEnvironment.canChangeSnapshots))
    func aRealFailureStopsBeforeTheNextMutationAndRefreshesLocalStateOnce() async throws {
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        let (process, url) = try await RealHTTPObserver.start(target: #require(NativeTestEnvironment.snapshotEndpoint), folder: folder)
        defer { process.terminate(); process.waitUntilExit() }
        let store = NativeStore(baseURL: url)
        let completed = await store.performSnapshotBatch(ids: ["not-a-snapshot", "another-invalid-id"], backup: false)
        #expect(completed.isEmpty)
        #expect(store.error != nil)
        let counts = try await store.api.request("/__native_counts")
        #expect(counts["DELETE /api/snapshots/not-a-snapshot"].integer == 1)
        #expect(counts["DELETE /api/snapshots/another-invalid-id"].integer == 0)
        #expect(counts["GET /api/snapshots"].integer == 1)
        #expect(counts["GET /api/storage"].integer == 1)
        #expect(counts["GET /api/backups"].integer == 0)
    }

    @MainActor
    @Test(.enabled(if: NativeTestEnvironment.canChangeSnapshots && fixture != nil))
    func realBatchDeletesOnlyCreatedCopiesAndRefreshesLocalStorageOnce() async throws {
        let target = try #require(NativeTestEnvironment.snapshotEndpoint)
        let originalID = try #require(Self.fixture)
        let api = NativeAPI(baseURL: target)
        let original = try await api.request("/api/snapshots").items
        try #require(original.contains { $0.id == originalID })
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        let archive = folder.appendingPathComponent("batch.tar.zst")
        try await api.download("/api/snapshots/\(originalID)/export", to: archive)
        var created: [String] = []
        do {
            for _ in 0..<2 {
                let copy = try await api.upload("/api/snapshots/import", file: archive)
                try #require(!copy.id.isEmpty && copy.id != originalID)
                created.append(copy.id)
            }
            let (process, observerURL) = try await RealHTTPObserver.start(target: target, folder: folder)
            defer { process.terminate(); process.waitUntilExit() }
            let store = NativeStore(baseURL: observerURL)
            let completed = await store.performSnapshotBatch(ids: created, backup: false)
            #expect(completed == created)
            #expect(store.error == nil)
            #expect(!store.snapshots.contains { created.contains($0.id) })
            #expect(store.snapshots.contains { $0.id == originalID })
            let counts = try await store.api.request("/__native_counts")
            for id in created { #expect(counts["DELETE /api/snapshots/\(id)"].integer == 1) }
            #expect(counts["GET /api/snapshots"].integer == 1)
            #expect(counts["GET /api/storage"].integer == 1)
            #expect(counts["GET /api/backups"].integer == 0)
            #expect(counts["GET /api/status"].integer == 0)
            #expect(counts["GET /api/tasks"].integer == 0)
        } catch {
            for id in created { _ = try? await api.request("/api/snapshots/\(id)", method: "DELETE") }
            throw error
        }
        for id in created { _ = try? await api.request("/api/snapshots/\(id)", method: "DELETE") }
    }

}
