import Testing
@testable import AgentVMNative

@Suite("Native recorded event delivery")
struct SSEChannelTests {
    @Test("Ended streams retain every initial event and stop only on the real comment boundary")
    func endedHistory() async throws {
        let channel = NativeAgentOutputChannel(buffer: NativeAgentOutputBuffer())
        for index in 1...5 {
            let keepReading = await channel.receive(.event(NativeSSEEvent(id: String(index), type: "agent",
                data: .object(["kind": JSON("text"), "text": JSON("line \(index)")]))), ended: true)
            #expect(keepReading)
        }
        let keepReading = await channel.receive(.comment(""), ended: true)
        #expect(!keepReading)
        let snapshot = try #require(await channel.snapshot())
        #expect(snapshot.loaded)
        #expect(snapshot.buffer.entries.map(\.text) == ["line 1", "line 2", "line 3", "line 4", "line 5"])
    }

    @Test("A live stream stays open across keepalive comments")
    func liveComments() async {
        let channel = NativeAgentOutputChannel(buffer: NativeAgentOutputBuffer())
        let keepReading = await channel.receive(.comment(""), ended: false)
        #expect(keepReading)
    }

    @Test("Empty reconnect history replaces stale cached output at its actual boundary")
    func emptyHistory() async throws {
        var cache = NativeAgentOutputBuffer()
        _ = cache.append(NativeSSEEvent(id: "1", type: "agent", data: .object(["kind": JSON("text"), "text": JSON("old")])))
        let channel = NativeAgentOutputChannel(buffer: cache)
        let initial = try #require(await channel.snapshot(force: true))
        #expect(initial.buffer.entries.map(\.text) == ["old"])
        #expect(!initial.loaded)
        _ = await channel.receive(.comment(""), ended: true)
        let fresh = try #require(await channel.snapshot())
        #expect(fresh.loaded)
        #expect(fresh.buffer.entries.isEmpty)
    }
}
