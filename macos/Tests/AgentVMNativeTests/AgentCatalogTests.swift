import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native agent catalog compatibility")
struct AgentCatalogTests {
    private static var legacyEndpoint: URL? {
        guard let raw = ProcessInfo.processInfo.environment["AGENTVM_NATIVE_LEGACY_TEST_URL"],
              let url = URL(string: raw), url.scheme == "http",
              ["127.0.0.1", "localhost"].contains(url.host ?? ""),
              let port = url.port, port != 7777 else { return nil }
        return url
    }

    @MainActor
    @Test(.enabled(if: legacyEndpoint != nil))
    func aRealLegacyEngineKeepsClaudeAvailableWithoutAnActionError() async throws {
        let store = NativeStore(baseURL: try #require(Self.legacyEndpoint))
        await store.refresh()
        try #require(store.connected)
        await store.refreshAgents()
        #expect(store.agentCatalogState == .legacy)
        #expect(store.agentCatalog.isNull)
        #expect(store.connected)
        #expect(store.error == nil)
    }

    @MainActor
    @Test
    func aRealUnavailableCatalogPreservesTheWorkspaceAndItsActionError() async {
        let store = NativeStore(baseURL: URL(string: "http://127.0.0.1:1")!)
        store.error = "Save failed"
        await store.refreshAgents()
        guard case .failed(let message) = store.agentCatalogState else {
            Issue.record("An unavailable catalog must report its own failure")
            return
        }
        #expect(!message.isEmpty)
        #expect(store.error == "Save failed")
    }
}
