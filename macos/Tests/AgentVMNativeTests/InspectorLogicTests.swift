import Foundation
import Testing
@testable import AgentVMNative

@Suite("Native inspector data")
struct InspectorLogicTests {
    @Test("Diagnostics open the hinted setup or console log and fall back to an existing log")
    func firstLog() {
        var diagnostics = JSON.object([
            "hint": .string("Read .agentvm/setup.sh output in setup.log"),
            "logs": .array([
                .object(["file": .string("share/job.log")]),
                .object(["file": .string("share/setup.log")]),
                .object(["file": .string("console.log")])
            ])
        ])
        #expect(NativeInspectorLogic.firstLog(diagnostics) == "share/setup.log")
        diagnostics["hint"] = .string("Read the last lines in console.log")
        #expect(NativeInspectorLogic.firstLog(diagnostics) == "console.log")
        diagnostics["logs"] = .array([.object(["file": .string("share/job.log")])])
        #expect(NativeInspectorLogic.firstLog(diagnostics) == "share/job.log")
        diagnostics["logs"] = .array([])
        #expect(NativeInspectorLogic.firstLog(diagnostics) == nil)
    }

    @Test("Cursor query parameters preserve arbitrary session names")
    func cursors() throws {
        let cursor = JSON.object(["a session&file.jsonl": .number(123)])
        let query = try NativeInspectorLogic.cursorQuery(cursor)
        let decoded = try JSONDecoder().decode(JSON.self, from: Data(query.removingPercentEncoding!.utf8))
        #expect(decoded["a session&file.jsonl"].integer == 123)
        #expect(NativeInspectorLogic.percentQuery("%&+=") == "%25%26%2B%3D")
    }

    @Test("Repeated blocks count one model call, including fractional timestamps")
    func calls() {
        let usage = JSON.object(["output_tokens": .number(12)])
        let entry = JSON.object(["at": .string("2026-10-14T15:01:02.123Z"), "message_id": .string("message-1"), "usage": usage])
        let turns = NativeInspectorLogic.turns([entry, entry])
        #expect(turns.count == 1)
        #expect(turns.first?["output_tokens"].integer == 12)
        #expect((turns.first?["at"].number ?? 0) > 0)
    }

    @Test("Reports retain timeline and every log source")
    func reports() {
        let report = JSON.object([
            "summary": .string("Stopped"), "hint": .string("Read console.log"),
            "timeline": .array([.object(["at": .number(1), "state": .string("booting")])]),
            "logs": .array([.object(["name": .string("Console"), "file": .string("console.log"), "tail": .string("boot output")])]),
            "server_log": .array([.string("process exited")])
        ])
        let text = NativeInspectorLogic.report(id: "machine-1", diagnostics: report)
        #expect(text.contains("machine-1"))
        #expect(text.contains("1970-01-01T00:00:01Z"))
        #expect(text.contains("boot output"))
        #expect(text.contains("process exited"))
    }
}
