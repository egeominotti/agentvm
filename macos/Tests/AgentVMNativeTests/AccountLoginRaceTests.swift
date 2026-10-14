import Combine
import Foundation
import Testing
@testable import AgentVMNative

@MainActor
@Suite("Native account login response ownership")
struct AccountLoginRaceTests {
    private func session(_ id: String) throws -> NativeAccountLoginSession {
        try NativeAccountLoginPolicy.session(response: .object([
            "id": .string(id), "verification_url": .string("https://auth.example.com/device"),
            "expires_in": JSON(900)
        ]), now: Date())
    }

    private func response(_ id: String, status: String) -> JSON {
        .object(["id": .string(id), "status": .string(status)])
    }

    @Test
    func aLateResponseCannotCompleteOrPublishOverAReplacementLogin() throws {
        let login = NativeAccountLoginState()
        login.begin(try session("old-login"))
        let replacement = try session("new-login")
        login.begin(replacement)
        login.polls = 4
        var publications = 0
        let observation = login.objectWillChange.sink { publications += 1 }
        #expect(!login.acceptResponse(response("old-login", status: "connected"), for: "old-login"))
        #expect(login.session == replacement)
        #expect(login.phase == .pending)
        #expect(login.polls == 4)
        #expect(publications == 0)
        withExtendedLifetime(observation) {}
    }

    @Test
    func aLateResponseCannotReviveAClearedLogin() throws {
        let login = NativeAccountLoginState()
        login.begin(try session("old-login"))
        login.clear()
        var publications = 0
        let observation = login.objectWillChange.sink { publications += 1 }
        #expect(!login.acceptResponse(response("old-login", status: "connected"), for: "old-login"))
        #expect(login.session == nil)
        #expect(login.phase == .pending)
        #expect(login.polls == 0)
        #expect(publications == 0)
        withExtendedLifetime(observation) {}
    }

    @Test
    func staleNetworkExpiryAndPollingLimitFailuresLeaveTheCurrentLoginUntouched() throws {
        let login = NativeAccountLoginState()
        let replacement = try session("new-login")
        login.begin(replacement)
        login.polls = 2
        var publications = 0
        let observation = login.objectWillChange.sink { publications += 1 }
        for error in ["Network failed", "Sign-in expired", "Polling limit reached"] {
            #expect(!login.setPhase(.failed(error), for: "old-login"))
            #expect(login.phase == .pending)
        }
        #expect(login.session == replacement)
        #expect(login.polls == 2)
        #expect(publications == 0)
        withExtendedLifetime(observation) {}
    }

    @Test
    func currentResponsesStillConnectWithoutPublishingUnchangedPendingStatuses() throws {
        let login = NativeAccountLoginState()
        login.begin(try session("current-login"))
        var publications = 0
        let observation = login.objectWillChange.sink { publications += 1 }
        #expect(login.acceptResponse(response("current-login", status: "pending"), for: "current-login"))
        #expect(publications == 0)
        #expect(login.acceptResponse(response("current-login", status: "connected"), for: "current-login"))
        #expect(login.phase == .connected)
        #expect(publications == 1)
        withExtendedLifetime(observation) {}
    }
}
