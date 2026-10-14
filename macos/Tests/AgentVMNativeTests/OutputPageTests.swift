import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native completed agent output pages")
struct OutputPageTests {
    @Test("Finite state and agent records retain server sequence IDs and content")
    func records() throws {
        let decoded = try NativeOutputPage(page([item(1, state: "running"), item(2, text: "actual output")], next: 2), cursor: 0)
        #expect(decoded.events == [NativeSSEEvent(id: "1", type: "state", data: .object(["state": JSON("running")])),
                                   NativeSSEEvent(id: "2", type: "agent", data: .object(["kind": JSON("text"), "text": JSON("actual output")]))])
        #expect(decoded.nextCursor == 2)
        #expect(!decoded.more)
    }

    @Test("Pagination advances through every page and preserves reported gaps")
    func pagination() throws {
        let first = try NativeOutputPage(page([item(4, text: "first")], next: 4, more: true, dropped: 3), cursor: 0)
        let next = try NativeOutputPage(page([item(7, text: "last")], next: 7, dropped: 2), cursor: first.nextCursor)
        #expect(first.more)
        #expect(first.nextCursor == 4)
        #expect(first.dropped == 3)
        #expect(!next.more)
        #expect(next.events.first?.id == "7")
        #expect(next.nextCursor == 7)
        #expect(next.dropped == 2)
    }

    @Test("Invalid pagination cannot repeat forever or silently skip ordered records")
    func invalidProgress() {
        for value in [page([], next: 0, more: true), page([item(2, text: "new")], next: 1),
                      page([item(1, text: "old")], next: 1), page([item(3, text: "a"), item(2, text: "b")], next: 3)] {
            #expect(throws: (any Error).self) { _ = try NativeOutputPage(value, cursor: 1) }
        }
    }

    @Test("Malformed cursor, gap, item and continuation fields are rejected")
    func malformed() {
        let valid = page([item(1, text: "output")], next: 1)
        for (field, wrong) in [("next_cursor", JSON(-1)), ("next_cursor", JSON(1.5)), ("dropped", JSON(-1)),
                               ("more", JSON("yes")), ("items", JSON("output"))] {
            var value = valid; value[field] = wrong
            #expect(throws: (any Error).self) { _ = try NativeOutputPage(value, cursor: 0) }
        }
        #expect(throws: (any Error).self) { _ = try NativeOutputPage(.object([:]), cursor: 0) }
    }

    @Test("Server gaps and client evictions are each counted once across pages")
    func omissions() throws {
        var buffer = NativeAgentOutputBuffer(countLimit: 1)
        buffer.beginConnection()
        let first = try NativeOutputPage(page([item(4, text: "a"), item(5, text: "b")], next: 5, more: true, dropped: 3), cursor: 0)
        _ = buffer.append(first)
        #expect(buffer.entries.map(\.text) == ["b"])
        #expect(buffer.dropped == 4)
        let next = try NativeOutputPage(page([item(8, text: "c")], next: 8, dropped: 2), cursor: 5)
        _ = buffer.append(next)
        #expect(buffer.entries.map(\.text) == ["c"])
        #expect(buffer.dropped == 7)
        _ = buffer.append(next)
        #expect(buffer.dropped == 7)
    }

    @Test("An empty completed history clears stale cached output only after its response")
    func emptyHistory() throws {
        var buffer = NativeAgentOutputBuffer()
        _ = buffer.append(NativeSSEEvent(id: "1", type: "agent", data: .object(["kind": JSON("text"), "text": JSON("old")])))
        buffer.beginConnection()
        #expect(buffer.entries.map(\.text) == ["old"])
        let empty = try NativeOutputPage(page([], next: 3, dropped: 3), cursor: 0)
        _ = buffer.append(empty)
        #expect(buffer.entries.isEmpty)
        #expect(buffer.dropped == 3)
    }

    @Test("Only an unsupported legacy endpoint triggers SSE fallback")
    func legacyFallback() {
        #expect(NativeOutputPage.usesLegacyStream(NativeAPIError(status: 404, message: "not found")))
        for status in [400, 401, 500, 502] {
            #expect(!NativeOutputPage.usesLegacyStream(NativeAPIError(status: status, message: "failure")))
        }
        #expect(!NativeOutputPage.usesLegacyStream(CancellationError()))
        #expect(!NativeOutputPage.usesLegacyStream(URLError(.cannotConnectToHost)))
    }

    private func item(_ seq: Int, text: String) -> JSON {
        .object(["seq": JSON(seq), "item": .object(["type": JSON("agent"), "data": .object(["kind": JSON("text"), "text": JSON(text)])])])
    }
    private func item(_ seq: Int, state: String) -> JSON {
        .object(["seq": JSON(seq), "item": .object(["type": JSON("state"), "data": .object(["state": JSON(state)])])])
    }
    private func page(_ items: [JSON], next: Int, more: Bool = false, dropped: Int = 0) -> JSON {
        .object(["items": .array(items), "next_cursor": JSON(next), "more": JSON(more), "dropped": JSON(dropped)])
    }
}
