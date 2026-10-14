import Testing
@testable import AgentVMNative

@Suite("Terminal renderer readiness")
struct TerminalStateTests {
    @Test("A renderer restart retains appearance and queues new pastes without replaying delivered input")
    func restart() {
        var state = NativeTerminalState()
        _ = state.setTheme("dark")
        _ = state.rendererReady()
        let delivered = state.paste("already delivered")
        #expect(delivered == [.paste("already delivered")])
        state.rendererReset()
        let duringReload = state.paste("new input")
        #expect(duringReload.isEmpty)
        let reloaded = state.rendererReady()
        #expect(reloaded == [.theme("dark"), .paste("new input")])
        let repeated = state.rendererReady()
        #expect(repeated.isEmpty)
    }

    @Test("Restarting before readiness preserves the pending paste and its byte budget")
    func restartBeforeReadiness() {
        var state = NativeTerminalState()
        _ = state.setTheme("dark")
        let original = String(repeating: "x", count: 600_000)
        _ = state.paste(original)
        state.rendererReset()
        let overflow = state.paste(String(repeating: "x", count: 600_000))
        #expect(overflow == [.pasteRejected])
        let reloaded = state.rendererReady()
        #expect(reloaded == [.theme("dark"), .paste(original)])
    }

    @Test("Readiness replays the latest desired appearance and queued paste once")
    func readiness() {
        var state = NativeTerminalState()
        let dark = state.setTheme("dark")
        let light = state.setTheme("light")
        let early = state.paste("queued")
        #expect(dark.isEmpty)
        #expect(light.isEmpty)
        #expect(early.isEmpty)
        let ready = state.rendererReady()
        #expect(ready == [.theme("light"), .paste("queued")])
        let repeated = state.rendererReady()
        #expect(repeated.isEmpty)
        let laterTheme = state.setTheme("dark")
        let laterPaste = state.paste("ready")
        #expect(laterTheme == [.theme("dark")])
        #expect(laterPaste == [.paste("ready")])
    }

    @Test("An oversized pending paste is rejected and never emitted after readiness")
    func pendingLimit() {
        var state = NativeTerminalState()
        _ = state.setTheme("light")
        let rejected = state.paste(String(repeating: "x", count: (1 << 20) + 1))
        #expect(rejected == [.pasteRejected])
        let ready = state.rendererReady()
        #expect(ready == [.theme("light")])
    }

    @Test("Pending paste limits count UTF-8 bytes across all queued chunks")
    func combinedLimit() {
        var state = NativeTerminalState()
        _ = state.setTheme("dark")
        let first = String(repeating: "a", count: 600_000)
        let accepted = state.paste(first)
        #expect(accepted.isEmpty)
        let rejected = state.paste(String(repeating: "é", count: 300_000))
        #expect(rejected == [.pasteRejected])
        let ready = state.rendererReady()
        #expect(ready == [.theme("dark"), .paste(first)])
    }
}
