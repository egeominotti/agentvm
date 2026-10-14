import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native terminal socket buffering")
struct SocketStateTests {
    @Test("Input waits for the WebSocket handshake and drains in FIFO order")
    func handshake() {
        var state = NativeTerminalSocketState()
        let acceptedFirst = state.enqueueInput(Data("first".utf8))
        let acceptedSecond = state.enqueueInput(Data("second".utf8))
        let beforeOpen = state.nextAttempt()
        #expect(acceptedFirst)
        #expect(acceptedSecond)
        #expect(beforeOpen == nil)
        state.connectionOpened()
        let first = state.nextAttempt()
        let second = state.nextAttempt()
        #expect(first == .input(Data("first".utf8)))
        #expect(second == .input(Data("second".utf8)))
        let exhausted = state.nextAttempt()
        #expect(exhausted == nil)
    }

    @Test("Reconnect keeps only input never handed to a send attempt")
    func reconnect() {
        var state = NativeTerminalSocketState()
        state.connectionOpened()
        _ = state.enqueueInput(Data("attempted".utf8))
        _ = state.enqueueInput(Data("not attempted".utf8))
        let attempted = state.nextAttempt()
        #expect(attempted == .input(Data("attempted".utf8)))
        state.connectionClosed()
        _ = state.enqueueInput(Data("during reconnect".utf8))
        let whileClosed = state.nextAttempt()
        #expect(whileClosed == nil)
        state.connectionOpened()
        let retained = state.nextAttempt()
        let added = state.nextAttempt()
        #expect(retained == .input(Data("not attempted".utf8)))
        #expect(added == .input(Data("during reconnect".utf8)))
        let exhausted = state.nextAttempt()
        #expect(exhausted == nil)
    }

    @Test("Resize bursts keep one latest size without starving queued input")
    func resizeCoalescing() {
        var state = NativeTerminalSocketState()
        for size in 80...1000 { state.resize(cols: size, rows: 24) }
        _ = state.enqueueInput(Data("command".utf8))
        state.connectionOpened()
        let latest = state.nextAttempt()
        #expect(latest == .resize(cols: 1000, rows: 24))
        state.resize(cols: 1200, rows: 30)
        let input = state.nextAttempt()
        #expect(input == .input(Data("command".utf8)))
        let resized = state.nextAttempt()
        #expect(resized == .resize(cols: 1200, rows: 30))
        let exhausted = state.nextAttempt()
        #expect(exhausted == nil)
        state.connectionClosed()
        state.connectionOpened()
        let restoredSize = state.nextAttempt()
        #expect(restoredSize == .resize(cols: 1200, rows: 30))
    }

    @Test("The pending input bound counts bytes and frees capacity only on attempt")
    func byteBound() {
        var state = NativeTerminalSocketState(limit: 8)
        let firstAccepted = state.enqueueInput(Data("éé".utf8))
        let secondAccepted = state.enqueueInput(Data("abcd".utf8))
        let overflowAccepted = state.enqueueInput(Data("x".utf8))
        #expect(firstAccepted)
        #expect(secondAccepted)
        #expect(!overflowAccepted)
        state.connectionOpened()
        let first = state.nextAttempt()
        #expect(first == .input(Data("éé".utf8)))
        let laterAccepted = state.enqueueInput(Data("next".utf8))
        #expect(laterAccepted)
        let second = state.nextAttempt()
        let third = state.nextAttempt()
        #expect(second == .input(Data("abcd".utf8)))
        #expect(third == .input(Data("next".utf8)))
    }

    @Test("Dismantling discards pending work and forbids later input")
    func close() {
        var state = NativeTerminalSocketState()
        _ = state.enqueueInput(Data("pending".utf8))
        state.resize(cols: 80, rows: 24)
        state.close()
        state.connectionOpened()
        let lateAccepted = state.enqueueInput(Data("late".utf8))
        let afterClose = state.nextAttempt()
        #expect(!lateAccepted)
        #expect(afterClose == nil)
    }
}
