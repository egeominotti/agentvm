import Foundation
import Testing
@testable import AgentVMNative

@Suite("NativeStoreTests")
struct NativeStoreTests {
    @MainActor
    @Test(.enabled(if: NativeTestEnvironment.outageEndpoint != nil))
    func localSnapshotPollingDoesNotContactUnavailableS3() async throws {
        let store = NativeStore(baseURL: try #require(NativeTestEnvironment.outageEndpoint))
        await store.refresh()
        try #require(store.connected)
        try #require(!store.settings["s3"].isNull)
        await store.refreshSnapshots(includeBackups: false)
        #expect(store.backupsError == nil)
        #expect(!store.backupsLoading)
        #expect(!store.storage.isNull)
        #expect(store.error == nil)
    }

    @MainActor
    @Test
    func aRealDisconnectedEndpointPreservesActionErrorsWithoutRepeatingModals() async {
        let store = NativeStore(baseURL: URL(string: "http://127.0.0.1:1")!)
        store.error = "Save failed: index.lock exists"
        await store.refresh()
        #expect(!store.connected)
        #expect(store.connectionMessage != nil)
        #expect(store.error == "Save failed: index.lock exists")
        store.error = nil
        await store.refresh()
        #expect(!store.connected)
        #expect(store.connectionMessage != nil)
        #expect(store.error == nil)
    }

    @MainActor
    @Test(.enabled(if: NativeTestEnvironment.outageEndpoint != nil))
    func realS3OutageStaysInlineAndKeepsLocalSnapshotsAvailable() async throws {
        let store = NativeStore(baseURL: try #require(NativeTestEnvironment.outageEndpoint))
        await store.refresh()
        try #require(store.connected)
        try #require(!store.settings["s3"].isNull)
        // This endpoint is read-only, including when it points at the user's running engine.
        await store.refreshSnapshots()
        #expect(store.error == nil)
        #expect(store.backupsError != nil)
        #expect(!store.backupsLoading)
        #expect(!store.storage.isNull)
        let first = store.backupsError
        await store.refreshSnapshots()
        #expect(store.error == nil)
        #expect(store.backupsError == first)
    }

    @MainActor
    @Test(.enabled(if: NativeTestEnvironment.endpoint != nil))
    func realSandboxWithoutS3KeepsSnapshotRefreshFreeOfErrors() async throws {
        let store = NativeStore(baseURL: try #require(NativeTestEnvironment.endpoint))
        await store.refresh()
        try #require(store.connected)
        try #require(store.settings["s3"].isNull)
        await store.refreshSnapshots()
        #expect(store.backups.isEmpty)
        #expect(store.error == nil)
        #expect(!store.storage.isNull)
    }

    @MainActor
    @Test(.enabled(if: NativeTestEnvironment.canChangeSettings))
    func testRealSandboxSettingsQueueKeepsLastDraftAndUntouchedFields() async throws {
        let base = try #require(NativeTestEnvironment.endpoint)
        let store = NativeStore(baseURL: base)
        await store.refresh()
        #expect(store.connected)
        let original = store.settings
        #expect(!(original.isNull))
        var first = original
        first["max_vms"] = JSON(original["max_vms"].integer == 1 ? 2 : 1)
        var second = first
        second["auto_snapshots"]["before_close"] = JSON(!original["auto_snapshots"]["before_close"].flag)
        let firstSave = store.queueSettings(first)
        let secondSave = store.queueSettings(second)
        await firstSave.value
        await secondSave.value
        let saved: JSON
        do { saved = try await store.api.request("/api/settings")["settings"] }
        catch {
            _ = try? await store.api.request("/api/settings", method: "PUT", body: original)
            throw error
        }
        _ = try await store.api.request("/api/settings", method: "PUT", body: original)
        #expect(saved["max_vms"] == second["max_vms"])
        #expect(saved["auto_snapshots"] == second["auto_snapshots"])
        #expect(saved["s3"] == original["s3"])
        #expect(saved["tailscale"] == original["tailscale"])
        #expect(saved["memory_mb"] == original["memory_mb"])
        #expect(store.settings == second)
        #expect(!(store.busy.contains("/api/settings")))
    }
}
