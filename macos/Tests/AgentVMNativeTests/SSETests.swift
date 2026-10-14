import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native bounded event stream")
struct SSETests {
    @Test("UTF-8, CRLF, comments and multiline JSON retain actual event fields")
    func multiline() throws {
        let values = try records("\u{feff}: heartbeat\r\nid: 12\r\nevent: agent\r\ndata: {\"kind\":\"text\",\r\ndata: \"text\":\"café 😀\"}\r\n\r\n")
        #expect(values == [.comment(" heartbeat"), .event(NativeSSEEvent(id: "12", type: "agent",
            data: .object(["kind": JSON("text"), "text": JSON("café 😀")])))] )
    }

    @Test("An incomplete frame is never emitted and comments do not erase its data")
    func incomplete() throws {
        let partial = try records("id: 2\nevent: state\ndata: {\"state\":\"running\"}\n")
        #expect(partial.isEmpty)
        let complete = try records("id: 2\nevent: state\ndata: {\"state\":\"running\"}\n:\n\n")
        #expect(complete.count == 2)
        #expect(complete.first == .comment(""))
        #expect(complete.last == .event(NativeSSEEvent(id: "2", type: "state", data: .object(["state": JSON("running")]))))
    }

    @Test("Lines and combined message data are bounded before decoding")
    func bounds() {
        #expect(throws: (any Error).self) { _ = try records("data: " + String(repeating: "x", count: 40) + "\n\n", limit: 32) }
        #expect(throws: (any Error).self) { _ = try records(String(repeating: "data: 12345678901234567890\n", count: 3) + "\n", limit: 32) }
        #expect(throws: (any Error).self) { _ = try records("data: {invalid}\n\n") }
    }

    @Test("Legal sixteen-KiB text with worst JSON escaping remains intact")
    func escapedText() throws {
        let text = String(repeating: "\u{1}", count: 16 << 10)
        let payload = try JSONEncoder().encode(JSON.object(["kind": JSON("text"), "text": JSON(text)]))
        try #require(payload.count > 64 << 10)
        let values = try records("id: 1\nevent: agent\ndata: " + String(decoding: payload, as: UTF8.self) + "\n\n")
        let preserved = values == [.event(NativeSSEEvent(id: "1", type: "agent", data: .object(["kind": JSON("text"), "text": JSON(text)])))]
        #expect(preserved)
    }

    @Test("Event IDs persist when omitted and null-containing IDs are ignored")
    func identifiers() throws {
        let values = try records("id: 7\ndata: {}\n\nid: bad\u{0}id\ndata: {}\n\n")
        #expect(values == [.event(NativeSSEEvent(id: "7", type: "message", data: .object([:]))),
                           .event(NativeSSEEvent(id: "7", type: "message", data: .object([:])))])
    }

    @Test("Fresh reconnect history replaces cached output without duplicating event IDs")
    func reconnect() {
        var buffer = NativeAgentOutputBuffer()
        _ = buffer.append(event(1, text: "old"))
        buffer.beginConnection()
        #expect(buffer.entries.map(\.text) == ["old"])
        _ = buffer.append(event(1, text: "old"))
        _ = buffer.append(event(1, text: "old"))
        _ = buffer.append(event(2, text: "new"))
        #expect(buffer.entries.map(\.text) == ["old", "new"])
        buffer.beginConnection()
        _ = buffer.append(NativeSSEEvent(id: "1", type: "state", data: .object(["state": JSON("closed")])))
        #expect(buffer.entries.isEmpty)
        #expect(buffer.state == "closed")
    }

    @Test("Rendering keeps the latest five hundred events")
    func countBound() {
        var buffer = NativeAgentOutputBuffer()
        for index in 1...1000 { _ = buffer.append(event(index, text: "output")) }
        #expect(buffer.entries.count == 500)
        #expect(buffer.entries.first?.id == "501")
        #expect(buffer.entries.last?.id == "1000")
        #expect(buffer.dropped == 500)
    }

    @Test("Display bytes remain bounded and the latest output survives eviction")
    func byteBound() {
        var buffer = NativeAgentOutputBuffer(byteLimit: 80)
        for index in 1...10 { _ = buffer.append(event(index, text: String(repeating: "é", count: 15))) }
        let bytes = buffer.entries.reduce(0) { $0 + $1.id.utf8.count + $1.title.utf8.count + $1.text.utf8.count }
        #expect(bytes <= 80)
        #expect(buffer.entries.last?.id == "10")
        #expect(buffer.dropped > 0)
    }

    @Test("Actual tool failures and result text render without synthesized usage")
    func toolError() {
        var buffer = NativeAgentOutputBuffer()
        _ = buffer.append(NativeSSEEvent(id: "3", type: "agent", data: .object([
            "kind": JSON("tool_result"), "is_error": JSON(true), "summary": JSON("Permission denied")
        ])))
        _ = buffer.append(NativeSSEEvent(id: "4", type: "agent", data: .object([
            "kind": JSON("result"), "is_error": JSON(false), "text": JSON("Done"), "cost_usd": JSON(0)
        ])))
        #expect(buffer.entries.first?.isError == true)
        #expect(buffer.entries.map(\.text) == ["Permission denied", "Done"])
    }

    private func records(_ text: String, limit: Int = 256 << 10) throws -> [NativeSSERecord] {
        var parser = NativeSSEDecoder(limit: limit)
        return try text.utf8.compactMap { try parser.append($0) }
    }

    private func event(_ id: Int, text: String) -> NativeSSEEvent {
        NativeSSEEvent(id: String(id), type: "agent", data: .object(["kind": JSON("text"), "text": JSON(text)]))
    }
}
