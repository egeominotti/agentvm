import Testing
@testable import AgentVMNative

@Suite("Native shell close preservation")
struct ShellCloseTests {
    private var state: NativeRestoredSessions {
        NativeRestoredSessions(active: "shell-2", extras: ["shell-2", "shell-3"], mounted: ["claude", "shell-2", "shell-3"])
    }

    @Test("A failed DELETE preserves the active terminal, extra tabs and mounted connections")
    func failedClose() {
        let after = NativeMachineLogic.closedSessions(name: "shell-2", state: state, succeeded: false)
        #expect(after.active == "shell-2")
        #expect(after.extras == ["shell-2", "shell-3"])
        #expect(after.mounted == Set(["claude", "shell-2", "shell-3"]))
    }

    @Test("Closing the active extra shell mounts the primary shell and preserves its sibling")
    func activeClose() {
        let after = NativeMachineLogic.closedSessions(name: "shell-2", state: state, succeeded: true)
        #expect(after.active == "shell")
        #expect(after.extras == ["shell-3"])
        #expect(after.mounted == Set(["claude", "shell", "shell-3"]))
    }

    @Test("Closing an inactive extra shell leaves the selected connection intact")
    func inactiveClose() {
        let after = NativeMachineLogic.closedSessions(name: "shell-3", state: state, succeeded: true)
        #expect(after.active == "shell-2")
        #expect(after.extras == ["shell-2"])
        #expect(after.mounted == Set(["claude", "shell-2"]))
    }
}
