import Foundation
import Testing
@testable import AgentVMNative

@Suite("Settings save recovery")
struct SettingsRecoveryTests {
    @MainActor
    @Test(.enabled(if: NativeTestEnvironment.outageEndpoint?.port != nil && NativeTestEnvironment.outageEndpoint?.port != 7777))
    func aValidQueuedDraftClearsThePreviousRejectedDraftError() async throws {
        let store = NativeStore(baseURL: try #require(NativeTestEnvironment.outageEndpoint))
        await store.refresh()
        let original = store.settings
        try #require(!original.isNull)
        var invalid = original
        invalid["max_vms"] = JSON(0)
        let saved = store.queueSettings(invalid)
        for _ in 0..<100 {
            if store.busy.contains("/api/settings") { break }
            await Task.yield()
        }
        try #require(store.busy.contains("/api/settings"))
        _ = store.queueSettings(original)
        await saved.value
        #expect(store.settings == original)
        #expect(store.notice == "Settings saved.")
        #expect(store.error == nil)
        #expect(!store.busy.contains("/api/settings"))
        let persisted = try await store.api.request("/api/settings")["settings"]
        #expect(persisted == original)
    }
}
