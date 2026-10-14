import Testing
@testable import AgentVMNative

@Suite("Native background polling")
struct PollingPolicyTests {
    @Test("Inactive windows perform one fifth of the overview polls")
    func inactivePolling() {
        #expect(NativePollingPolicy.overviewSeconds(active: true) == 1)
        #expect(NativePollingPolicy.overviewSeconds(active: false) == 5)
    }
}
