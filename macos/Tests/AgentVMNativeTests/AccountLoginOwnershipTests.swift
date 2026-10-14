import Combine
import Foundation
import Testing
@testable import AgentVMNative

@MainActor
@Suite("Native account login ownership")
struct AccountLoginOwnershipTests {
    private func session(_ id: String) throws -> NativeAccountLoginSession {
        try NativeAccountLoginPolicy.session(response: .object([
            "id": .string(id), "verification_url": .string("https://auth.example.com/device"),
            "user_code": .string("ABCD-EFGH"), "expires_in": JSON(900)
        ]), now: Date())
    }

    @Test
    func returningToAccountSettingsKeepsThePublicLoginAndPollingProgress() throws {
        let store = NativeStore(baseURL: URL(string: "http://127.0.0.1:1")!)
        let beforeNavigation = store.accountLoginState("chatgpt")
        let expected = try session("existing-login")
        beforeNavigation.begin(expected)
        beforeNavigation.polls = 7
        let afterNavigation = store.accountLoginState("chatgpt")
        #expect(beforeNavigation === afterNavigation)
        #expect(afterNavigation.session == expected)
        #expect(afterNavigation.polls == 7)
        #expect(store.accountLoginState("google") !== afterNavigation)
        #expect(store.accountLoginState("google").session == nil)
    }

    @Test
    func unchangedPendingPollsDoNotPublishOrInvalidateTheWorkspace() throws {
        let store = NativeStore(baseURL: URL(string: "http://127.0.0.1:1")!)
        let login = store.accountLoginState("chatgpt")
        login.begin(try session("existing-login"))
        var loginChanges = 0
        var workspaceChanges = 0
        let local = login.objectWillChange.sink { loginChanges += 1 }
        let global = store.objectWillChange.sink { workspaceChanges += 1 }
        login.polls += 1
        login.setPhase(.pending)
        #expect(loginChanges == 0)
        #expect(workspaceChanges == 0)
        login.setPhase(.connected)
        #expect(loginChanges == 1)
        #expect(workspaceChanges == 0)
        withExtendedLifetime((local, global)) {}
    }
}
