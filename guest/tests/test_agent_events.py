import importlib.util
import pathlib
import unittest

GUEST = pathlib.Path(__file__).resolve().parents[1]


class AgentEvents(unittest.TestCase):
    def normalize(self, kind, event):
        path = GUEST / "agent_events.py"
        self.assertTrue(path.exists(), "the guest stream normalizer is missing")
        spec = importlib.util.spec_from_file_location("agent_events", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module.normalize(kind, event)

    def text(self, events):
        return "".join(block.get("text", "") for event in events
                       for block in event.get("message", {}).get("content", []))

    def test_claude_events_retain_usage_and_tool_structure(self):
        event = {"type": "result", "total_cost_usd": 0.1, "result": "done"}
        self.assertEqual(self.normalize("claude", event), [event])

    def test_codex_completed_message_is_visible(self):
        event = {"type": "item.completed", "item": {"type": "agent_message", "text": "Fixed."}}
        self.assertEqual(self.text(self.normalize("codex", event)), "Fixed.")

    def test_codex_command_has_actual_command_and_output(self):
        event = {"type": "item.completed", "item": {"id": "item_2", "type": "command_execution",
                 "command": "cargo test", "aggregated_output": "all green", "exit_code": 0}}
        events = self.normalize("codex", event)
        self.assertEqual(events[0]["message"]["content"][0]["input"], {"command": "cargo test"})
        self.assertEqual(events[1]["message"]["content"][0]["content"], "all green")

    def test_gemini_and_qwen_assistant_messages_are_visible(self):
        for kind in ["gemini", "qwen"]:
            events = self.normalize(kind, {"type": "message", "role": "assistant", "content": "hello"})
            self.assertEqual(self.text(events), "hello")

    def test_opencode_and_kilo_text_events_are_visible(self):
        for kind in ["opencode", "kilo"]:
            event = {"type": "text", "part": {"type": "text", "text": "hello"}}
            self.assertEqual(self.text(self.normalize(kind, event)), "hello")

    def test_other_agents_do_not_invent_claude_usage_or_prices(self):
        events = self.normalize("gemini", {"type": "result", "result": "done", "stats": {"tokens": 12}})
        self.assertEqual(self.text(events), "done")
        self.assertFalse(any(event.get("type") == "result" for event in events))

    def test_plain_custom_output_is_preserved(self):
        self.assertEqual(self.text(self.normalize("custom", "plain output")), "plain output")

    def test_unknown_structured_output_is_preserved_without_usage_claims(self):
        text = self.text(self.normalize("goose", {"type": "future_event", "body": "content"}))
        self.assertIn("future_event", text)
        self.assertIn("content", text)

    def test_goose_nested_assistant_message_is_visible(self):
        event = {"type": "message", "message": {"role": "assistant",
                 "content": [{"type": "text", "text": "Fixed."}]}}
        self.assertEqual(self.text(self.normalize("goose", event)), "Fixed.")

    def test_copilot_assistant_event_is_visible_without_replaying_deltas(self):
        event = {"type": "assistant.message", "data": {"messageId": "m1", "content": "Fixed."}}
        self.assertEqual(self.text(self.normalize("copilot", event)), "Fixed.")
        delta = {"type": "assistant.message_delta", "data": {"messageId": "m1", "deltaContent": "Fixed."}}
        self.assertEqual(self.normalize("copilot", delta), [])

    def test_gemini_tool_preserves_actual_parameters_and_failure(self):
        start = {"type": "tool_use", "tool_id": "t1", "tool_name": "run_shell_command", "parameters": {"command": "pwd"}}
        block = self.normalize("gemini", start)[0]["message"]["content"][0]
        self.assertEqual(block["name"], "run_shell_command")
        self.assertEqual(block["input"], {"command": "pwd"})
        result = {"type": "tool_result", "tool_id": "t1", "status": "error", "error": "failed"}
        block = self.normalize("gemini", result)[0]["message"]["content"][0]
        self.assertTrue(block["is_error"])
        self.assertEqual(block["content"], "failed")

    def test_copilot_tool_output_and_failure_keep_the_call_id(self):
        event = {"type": "tool.execution_complete", "data": {"toolCallId": "t1", "success": False,
                 "error": {"message": "failed"}}}
        block = self.normalize("copilot", event)[0]["message"]["content"][0]
        self.assertEqual(block["tool_use_id"], "t1")
        self.assertTrue(block["is_error"])
        self.assertEqual(block["content"], "failed")


if __name__ == "__main__":
    unittest.main()
