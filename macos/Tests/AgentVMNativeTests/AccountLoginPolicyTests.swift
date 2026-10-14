import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native finite account login")
struct AccountLoginPolicyTests {
    private let now = Date(timeIntervalSince1970: 1_800_000_000)
    private var start: JSON {
        .object(["id": .string("login-123"), "verification_url": .string("https://auth.example.com/device"),
                 "user_code": .string("ABCD-EFGH"), "expires_in": JSON(30)])
    }

    @Test
    func publicLoginProjectionKeepsItsCodeAndServerExpiry() throws {
        let session = try NativeAccountLoginPolicy.session(response: start, now: now)
        #expect(session.id == "login-123")
        #expect(session.verificationURL.absoluteString == "https://auth.example.com/device")
        #expect(session.userCode == "ABCD-EFGH")
        #expect(session.expiresAt == now.addingTimeInterval(30))
    }

    @Test
    func malformedSessionsCannotOpenAnUnsafeBrowserDestination() {
        for value in ["http://auth.example.com/device", "file:///tmp/auth", "javascript:login()",
                      "https://user:password@auth.example.com/device", "/device"] {
            var response = start
            response["verification_url"] = .string(value)
            #expect(throws: NativeAPIError.self) { try NativeAccountLoginPolicy.session(response: response, now: now) }
        }
        for id in ["", "../account", "login?id=other", "login#other"] {
            var response = start
            response["id"] = .string(id)
            #expect(throws: NativeAPIError.self) { try NativeAccountLoginPolicy.session(response: response, now: now) }
        }
        for expiry in [0.0, -1.0, .infinity, .nan] {
            var response = start
            response["expires_in"] = .number(expiry)
            #expect(throws: NativeAPIError.self) { try NativeAccountLoginPolicy.session(response: response, now: now) }
        }
    }

    @Test
    func aDifferentLoginCannotMarkTheCurrentAccountConnected() {
        let connected = JSON.object(["id": .string("other-login"), "status": .string("connected")])
        guard case .failed = NativeAccountLoginPolicy.phase(response: connected, id: "login-123") else {
            Issue.record("An unrelated login response must fail instead of connecting this session")
            return
        }
    }

    @Test
    func finiteStatusesPreservePendingConnectionAndActionableFailures() {
        func response(_ status: String, error: String? = nil) -> JSON {
            .object(["id": .string("login-123"), "status": .string(status), "error": error.map(JSON.string) ?? .null])
        }
        #expect(NativeAccountLoginPolicy.phase(response: response("pending"), id: "login-123") == .pending)
        #expect(NativeAccountLoginPolicy.phase(response: response("connected"), id: "login-123") == .connected)
        #expect(NativeAccountLoginPolicy.phase(response: response("failed", error: "Access denied"), id: "login-123") == .failed("Access denied"))
        guard case .failed = NativeAccountLoginPolicy.phase(response: response("unknown"), id: "login-123") else {
            Issue.record("An unknown status must not keep polling indefinitely")
            return
        }
    }

    @Test
    func pollingPausesWhenInactiveAndStopsAtExpiryOrItsFiniteBound() throws {
        let session = try NativeAccountLoginPolicy.session(response: start, now: now)
        #expect(NativeAccountLoginPolicy.pollDecision(session: session, phase: .pending, polls: 0, now: now, active: true) == .request)
        #expect(NativeAccountLoginPolicy.pollDecision(session: session, phase: .pending, polls: 0, now: now, active: false) == .paused)
        #expect(NativeAccountLoginPolicy.pollDecision(session: session, phase: .pending, polls: 0,
                                                       now: session.expiresAt, active: true) == .expired)
        #expect(NativeAccountLoginPolicy.pollDecision(session: session, phase: .pending, polls: 300, now: now, active: true) == .limitReached)
        #expect(NativeAccountLoginPolicy.pollDecision(session: session, phase: .connected, polls: 0, now: now, active: true) == .finished)
        #expect(NativeAccountLoginPolicy.pollDecision(session: session, phase: .failed("Denied"), polls: 0, now: now, active: true) == .finished)
    }
}
