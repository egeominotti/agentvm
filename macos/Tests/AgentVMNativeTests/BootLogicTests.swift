import Testing
@testable import AgentVMNative

@Suite("Native boot evidence")
struct BootLogicTests {
    @Test("Step timings come from host and guest events")
    func actualTimings() {
        let task = JSON.object([
            "status": .object(["state": .string("running")]), "interactive": .bool(true), "ready": .bool(true),
            "boot_log": .array([
                .string("host: disk ready in 35 ms"), .string("[0.1s] job start"), .string("[0.3s] repo ready"),
                .string("[0.5s] network ready"), .string("[1.2s] terminal ready")
            ])
        ])
        let steps = NativeBootLogic.steps(task)
        #expect(steps.allSatisfy { $0.done })
        #expect(steps[1].detail == "35 ms")
        #expect(steps.last?.detail == "1.2 s")
    }

    @Test("Failed setup is recorded without preventing terminal readiness")
    func setupFailure() {
        let task = JSON.object([
            "status": .object(["state": .string("running")]), "interactive": .bool(true), "ready": .bool(true),
            "boot_log": .array([.string("[0.2s] running .agentvm/setup.sh"), .string("[0.4s] setup failed (exit 1)")])
        ])
        #expect(NativeBootLogic.setupFailed(task))
        #expect(NativeBootLogic.steps(task).contains { $0.failed && $0.done })
        #expect(NativeBootLogic.steps(task).last?.done == true)
    }
}
