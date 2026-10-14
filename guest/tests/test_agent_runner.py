import importlib.util
import json
import os
import pathlib
import sys
import tempfile
import unittest

GUEST = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(GUEST))


class AgentRunner(unittest.TestCase):
    def module(self):
        path = GUEST / "agent_runner.py"
        self.assertTrue(path.exists(), "the generic guest runner is missing")
        spec = importlib.util.spec_from_file_location("agent_runner", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_actual_process_receives_literal_stdin_and_normalized_output(self):
        module = self.module()
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            prompt = "--help; $(touch should-not-exist)\n'quoted'"
            code = "import sys; print(sys.stdin.read()); print('done', file=sys.stderr)"
            exit_code = module.run_batch([sys.executable, "-c", code], dict(os.environ),
                                         prompt, "custom", root, root, use_stdin=True)
            self.assertEqual(exit_code, 0)
            self.assertEqual((root / "agent.log").read_text(), prompt + "\n")
            self.assertEqual((root / "agent.err").read_text(), "done\n")
            events = [json.loads(line) for line in (root / "stream.jsonl").read_text().splitlines()]
            text = "".join(event["message"]["content"][0]["text"] for event in events)
            self.assertEqual(text, prompt.replace("\n", ""))
            self.assertFalse((root / "should-not-exist").exists())

    def test_actual_failing_process_keeps_exit_status_and_codex_message(self):
        module = self.module()
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            event = {"type": "item.completed", "item": {"type": "agent_message", "text": "progress"}}
            code = "import sys; print(" + repr(json.dumps(event)) + "); sys.exit(7)"
            exit_code = module.run_batch([sys.executable, "-c", code], dict(os.environ),
                                         "", "codex", root, root)
            self.assertEqual(exit_code, 7)
            normalized = json.loads((root / "stream.jsonl").read_text())
            self.assertEqual(normalized["message"]["content"][0]["text"], "progress")

    def test_cached_automatic_model_reaches_interactive_claude_without_consuming_prompt(self):
        module = self.module()
        with tempfile.TemporaryDirectory() as directory:
            runtime = pathlib.Path(directory)
            (runtime / "resolved-model").write_text("provider-chat-model")
            (runtime / "prompt").write_text("first prompt")
            model = module.prepare_model({"agent": {"kind": "claude", "provider": "custom"}}, runtime)
            self.assertEqual(model, "provider-chat-model")
            self.assertEqual((runtime / "model").read_text(), "provider-chat-model")
            self.assertEqual((runtime / "prompt").read_text(), "first prompt")
            self.assertFalse((runtime / "prompt-consumed").exists())

    def test_account_model_refresh_does_not_reuse_an_api_alias_from_a_snapshot(self):
        module = self.module()
        account = {"agent": {"kind": "codex", "auth": "account"}}
        self.assertIsNone(module.model_to_prepare(account, "sonnet"))
        self.assertEqual(module.model_to_prepare({**account, "model": "explicit-override"}, "sonnet"), "explicit-override")
        self.assertEqual(module.model_to_prepare({"agent": {"kind": "codex", "auth": "api"}}, "cached-model"), "cached-model")

    def test_restored_native_auth_is_replaced_while_preferences_are_preserved(self):
        module = self.module()
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            auth = root / ".codex" / "auth.json"
            auth.parent.mkdir()
            auth.write_text(json.dumps({"tokens": {"access_token": "old-guest-secret"}}))
            module.write_config(auth, "{}")
            self.assertEqual(json.loads(auth.read_text()), {})
            self.assertEqual(auth.stat().st_mode & 0o777, 0o600)
            preferences = root / ".gemini" / "settings.json"
            preferences.parent.mkdir()
            preferences.write_text(json.dumps({"theme": "dark", "security": {"auth": {"selectedType": "oauth-personal"}}}))
            module.write_config(preferences, json.dumps({"security": {"auth": {"selectedType": "gemini-api-key"}}}))
            self.assertEqual(json.loads(preferences.read_text()), {"theme": "dark", "security": {"auth": {"selectedType": "gemini-api-key"}}})


if __name__ == "__main__":
    unittest.main()
