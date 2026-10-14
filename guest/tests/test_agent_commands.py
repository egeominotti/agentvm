import importlib.util
import pathlib
import sys
import unittest

GUEST = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(GUEST))


class AgentCommands(unittest.TestCase):
    def launch(self, kind, prompt="", model="test-model", interactive=True, restore=False, **agent):
        path = GUEST / "agent_commands.py"
        self.assertTrue(path.exists(), "the multi-CLI guest launcher is missing")
        module_spec = importlib.util.spec_from_file_location("agent_commands", path)
        module = importlib.util.module_from_spec(module_spec)
        module_spec.loader.exec_module(module)
        spec = {"agent": {"kind": kind, **agent}, "interactive": interactive,
                "agent_endpoint": "http://127.0.0.1:5001/v1", "agent_protocol": "openai"}
        return module.launch(spec, prompt, model, restore)

    def test_codex_headless_reads_prompt_from_stdin_without_nested_sandbox(self):
        args, env = self.launch("codex", "fix this", interactive=False)
        self.assertEqual(args, ["codex", "exec", "--dangerously-bypass-approvals-and-sandbox",
                               "--model", "test-model", "--json", "-"])
        self.assertEqual(env["AGENTVM_DUMMY_API_KEY"], "agentvm-dummy")

    def test_codex_resume_does_not_start_a_new_conversation(self):
        args, _ = self.launch("codex", restore=True, model=None)
        self.assertEqual(args, ["codex", "resume", "--last", "--dangerously-bypass-approvals-and-sandbox"])

    def test_prompts_beginning_with_flags_stay_literal(self):
        prompt = "--help; $(touch /tmp/owned)\n'quoted'"
        for kind, flag in [("gemini", "--prompt-interactive="), ("qwen", "--prompt-interactive="),
                           ("copilot", "--interactive="), ("opencode", "--prompt="), ("kilo", "--prompt=")]:
            with self.subTest(kind=kind):
                args, _ = self.launch(kind, prompt)
                self.assertIn(flag + prompt, args)

    def test_gemini_headless_uses_developer_api_and_no_container_sandbox(self):
        args, env = self.launch("gemini", "hello", interactive=False)
        self.assertEqual(args, ["gemini", "--approval-mode=yolo", "--model", "test-model",
                               "--prompt=hello", "--output-format", "stream-json"])
        self.assertEqual(env["GEMINI_SANDBOX"], "false")
        self.assertEqual(env["GEMINI_API_KEY"], "agentvm-dummy")

    def test_qwen_resume_and_headless_flags(self):
        args, _ = self.launch("qwen", "follow up", interactive=False, restore=True)
        self.assertEqual(args, ["qwen", "--yolo", "--model", "test-model", "--continue",
                               "--prompt=follow up", "--output-format", "stream-json"])

    def test_copilot_uses_offline_byok_and_the_selected_endpoint(self):
        args, env = self.launch("copilot", "fix", interactive=False)
        self.assertEqual(args, ["copilot", "--allow-all", "--no-ask-user", "--model", "test-model",
                               "--prompt=fix", "--output-format", "json"])
        self.assertEqual(env["COPILOT_OFFLINE"], "true")
        self.assertEqual(env["COPILOT_PROVIDER_BASE_URL"], "http://127.0.0.1:5001/v1")

    def test_opencode_and_kilo_headless_use_named_configured_model(self):
        for kind in ["opencode", "kilo"]:
            with self.subTest(kind=kind):
                args, _ = self.launch(kind, "- literal", interactive=False)
                self.assertEqual(args[:2], [kind, "run"])
                self.assertIn("agentvm/test-model", args)
                self.assertEqual(args[-2:], ["--", "- literal"])

    def test_aider_uses_message_file_and_can_restore_history(self):
        args, _ = self.launch("aider", "fix", interactive=False, restore=True)
        self.assertIn("openai/test-model", args)
        self.assertIn("--restore-chat-history", args)
        self.assertEqual(args[-2:], ["--message-file", "/run/agentvm/prompt"])

    def test_goose_named_session_supports_prompt_then_interactive(self):
        args, env = self.launch("goose", "hello", restore=True)
        self.assertEqual(args[:3], ["goose", "run", "--text=hello"])
        self.assertIn("-s", args)
        self.assertIn("--resume", args)
        self.assertIn("agentvm", args)
        self.assertEqual(env["GOOSE_MODE"], "auto")

    def test_custom_template_substitution_cannot_execute_prompt(self):
        custom = {"name": "My CLI", "interactive": "my-cli --model '{model}' --task '{prompt}'",
                  "headless": "my-cli --print --task '{prompt}'", "resume": "my-cli --continue",
                  "install": None}
        prompt = "a' $(touch /tmp/owned)\n--help"
        args, _ = self.launch("custom", prompt, custom=custom)
        self.assertEqual(args, ["my-cli", "--model", "test-model", "--task", prompt])

    def test_custom_without_headless_template_is_rejected(self):
        with self.assertRaises(ValueError):
            self.launch("custom", "fix", interactive=False,
                        custom={"name": "CLI", "interactive": "my-cli", "headless": None})

    def test_unknown_agent_is_rejected(self):
        with self.assertRaises(ValueError):
            self.launch("made-up", "hello")

    def test_claude_keeps_descriptor_auth_instead_of_a_dummy_api_key(self):
        _, env = self.launch("claude")
        self.assertNotIn("ANTHROPIC_API_KEY", env)
        self.assertEqual(env["ANTHROPIC_BASE_URL"], "http://127.0.0.1:5001/v1")

    def test_agents_requiring_a_model_report_a_clear_error(self):
        for kind in ["opencode", "qwen", "copilot", "kilo", "aider", "goose"]:
            with self.subTest(kind=kind), self.assertRaisesRegex(ValueError, "model"):
                self.launch(kind, model=None)

    def test_claude_explicit_provider_uses_gateway_auth_without_key_approval(self):
        _, env = self.launch("claude", provider="anthropic")
        self.assertEqual(env.get("ANTHROPIC_AUTH_TOKEN"), "agentvm-dummy")


if __name__ == "__main__":
    unittest.main()
