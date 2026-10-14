import Testing
@testable import AgentVMNative

@Suite("Native request timeouts")
struct RequestPolicyTests {
    @Test("Archive preparation can take longer than an ordinary dashboard GET")
    func exportTimeout() {
        #expect(NativeRequestPolicy.timeout(path: "/api/snapshots/snap-123/export", method: "GET") == 360)
        #expect(NativeRequestPolicy.timeout(path: "/api/snapshots/snap-123/export", method: "get") == 360)
    }

    @Test("Ordinary polls fail promptly while mutations retain their longer timeout")
    func overviewAndMutationTimeouts() {
        #expect(NativeRequestPolicy.timeout(path: "/api/status", method: "GET") == 30)
        #expect(NativeRequestPolicy.timeout(path: "/api/tasks/task-1/export", method: "GET") == 30)
        #expect(NativeRequestPolicy.timeout(path: "/api/snapshots/import", method: "POST") == 360)
        #expect(NativeRequestPolicy.timeout(path: "/api/tasks/task-1/close", method: "POST") == 360)
    }
}
