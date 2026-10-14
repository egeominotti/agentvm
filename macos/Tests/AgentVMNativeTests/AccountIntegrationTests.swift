import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native read-only account routes")
struct AccountIntegrationTests {
    private static var endpoint: URL? {
        guard let raw = ProcessInfo.processInfo.environment["AGENTVM_NATIVE_ACCOUNT_TEST_URL"],
              let url = URL(string: raw), url.scheme == "http",
              ["127.0.0.1", "localhost"].contains(url.host ?? ""),
              let port = url.port, port != 7777 else { return nil }
        return url
    }

    @MainActor
    @Test(.enabled(if: endpoint != nil))
    func realCatalogAndConnectionStatusesContainOnlyPublicAccountMetadata() async throws {
        let store = NativeStore(baseURL: try #require(Self.endpoint))
        await store.refreshAgents()
        try #require(store.agentCatalogState == .supported)
        try #require(NativeAgentAuthentication.supported(catalog: store.agentCatalog))
        let accounts = store.agentCatalog["accounts"].items
        #expect(Set(accounts.map(\.id)) == ["claude", "chatgpt", "google", "copilot", "kilo"])
        let publicKeys: Set<String> = ["id", "name", "connected", "source", "error", "can_import", "can_login"]
        for account in accounts {
            #expect(Set(account.fields.keys).isSubset(of: publicKeys))
            let status = try await store.api.request("/api/settings/accounts/" + account.id)
            #expect(status.id == account.id)
            #expect(!status["name"].text.isEmpty)
            #expect(Set(status.fields.keys).isSubset(of: publicKeys))
            guard case .bool = status["connected"], case .bool = status["can_import"],
                  case .bool = status["can_login"] else {
                Issue.record("Account status must include connection and action capabilities")
                return
            }
            if status.id == "claude" { #expect(!status["can_login"].flag) }
        }
    }

    @Test(.enabled(if: endpoint != nil))
    func anUnstartedLoginUsesTheRealFiniteStatusRouteWithoutBeginningAuthentication() async throws {
        let api = NativeAPI(baseURL: try #require(Self.endpoint))
        do {
            _ = try await api.request("/api/settings/accounts/chatgpt/login/native-read-only-unstarted")
            Issue.record("An unstarted session must fail on the finite account status route")
        } catch let failure as NativeAPIError {
            #expect(failure.status == 400)
            #expect(!failure.message.isEmpty)
        }
    }
}
