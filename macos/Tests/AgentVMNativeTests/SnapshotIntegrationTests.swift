import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native streamed snapshot lifecycle")
struct SnapshotIntegrationTests {
    private static var fixture: String? {
        ProcessInfo.processInfo.environment["AGENTVM_NATIVE_SNAPSHOT_ID"]
    }

    @Test(.enabled(if: NativeTestEnvironment.canChangeSnapshots && fixture != nil))
    func realArchiveExportImportAndDeletePreserveTheOriginal() async throws {
        let api = NativeAPI(baseURL: try #require(NativeTestEnvironment.snapshotEndpoint))
        let originalID = try #require(Self.fixture)
        let initial = try await api.request("/api/snapshots").items
        let original = try #require(initial.first { $0.id == originalID })
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }
        let archive = folder.appendingPathComponent("native-export.tar.zst")
        // Exercise successful replacement as well as URLSession's streamed file transfer.
        try Data("previous export".utf8).write(to: archive)
        try await api.download("/api/snapshots/\(originalID)/export", to: archive)
        let attributes = try FileManager.default.attributesOfItem(atPath: archive.path)
        #expect((attributes[.size] as? NSNumber)?.int64Value ?? 0 > 16)
        let imported = try await api.upload("/api/snapshots/import", file: archive)
        let createdID = imported.id
        // The fixture is never deleted, even if an invalid response reaches this assertion.
        try #require(!createdID.isEmpty && createdID != originalID)
        do {
            #expect(imported["repo"] == original["repo"])
            #expect(imported["base_sha"] == original["base_sha"])
            #expect(imported["name"] == original["name"])
            let listed = try await api.request("/api/snapshots").items
            #expect(listed.contains { $0.id == createdID })
            #expect(listed.contains { $0.id == originalID })
            _ = try await api.request("/api/snapshots/\(createdID)", method: "DELETE")
        } catch {
            _ = try? await api.request("/api/snapshots/\(createdID)", method: "DELETE")
            throw error
        }
        let final = try await api.request("/api/snapshots").items
        #expect(!final.contains { $0.id == createdID })
        #expect(final.contains { $0.id == originalID })
    }
}
