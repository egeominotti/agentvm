import Foundation
import Testing
@testable import AgentVMNative

@Suite("JSONTests")
struct JSONTests {
    @Test
    func testNestedSettingsEditsPreserveUnknownFieldsAndNulls() throws {
        let source = Data(#"{"memory_mb":4096,"s3":{"bucket":"work","future":true},"tailscale":{"enabled":false,"tags":["tag:agentvm"]},"future_setting":{"value":null}}"#.utf8)
        var settings = try JSONDecoder().decode(JSON.self, from: source)
        settings["memory_mb"] = JSON(8192)
        settings["tailscale"]["enabled"] = JSON(true)
        let saved = try JSONDecoder().decode(JSON.self, from: JSONEncoder().encode(settings))
        #expect(saved["memory_mb"].integer == 8192)
        #expect(saved["tailscale"]["enabled"].flag)
        #expect(saved["tailscale"]["tags"].items == [.string("tag:agentvm")])
        #expect(saved["s3"]["future"].flag)
        #expect(saved["future_setting"]["value"].isNull)
    }

    @Test
    func testScalarTypesRemainDistinctAndUnsafeIntegersNeverTrap() throws {
        let values = try JSONDecoder().decode(JSON.self, from: Data(#"[true,1,"1",null,1.25]"#.utf8)).items
        #expect(values == [.bool(true), .number(1), .string("1"), .null, .number(1.25)])
        #expect(JSON.number(Double(Int.max)).integer == 0)
        #expect(JSON.number(.infinity).integer == 0)
        #expect(JSON.number(12.9).integer == 12)
        #expect(throws: (any Error).self) { try JSONEncoder().encode(JSON.number(.nan)) }
    }

    @Test
    func testTaskTitlesAndTerminalStatesMatchServerLifecycle() {
        var task = JSON.object(["id": JSON("01900000-abcd-7000-8000-000000000000"), "repo": JSON("/work/shop/")])
        #expect(task.title == "shop")
        task["label"] = JSON("Spike")
        #expect(task.title == "Spike")
        task["prompt"] = JSON("Fix the cart\nand the tests")
        #expect(task.title == "Fix the cart")
        for state in ["done", "no_changes", "failed", "stopped"] {
            task["status"] = .object(["state": JSON(state)])
            #expect(task.isEnded)
        }
        task["status"] = .object(["state": JSON("collecting")])
        #expect(!(task.isEnded))
    }
}
