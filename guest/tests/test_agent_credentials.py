import json
import pathlib
import sys
import unittest

GUEST = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(GUEST))
import agent_auth
from agent_commands import environment
from agent_config import configs


class GuestCredentials(unittest.TestCase):
    def test_copilot_only_receives_documented_direct_dummy_token(self):
        spec = {"agent": {"kind": "copilot", "auth": "account"},
                "agent_endpoint": "http://127.0.0.1:5001"}
        env = environment(spec, "gpt-current")
        self.assertEqual(env.get("GITHUB_COPILOT_API_TOKEN"), "agentvm-dummy")
        for key in ("COPILOT_API_TOKEN", "GH_TOKEN", "GITHUB_TOKEN", "COPILOT_GITHUB_TOKEN"):
            self.assertNotIn(key, env)

    def test_both_copilot_api_token_spellings_are_removed_before_launch(self):
        env = {"COPILOT_API_TOKEN": "old", "GITHUB_COPILOT_API_TOKEN": "old",
               "COPILOT_API_URL": "https://old.invalid", "KEEP_ME": "value"}
        self.assertEqual(agent_auth.sanitize_environment(env), {"KEEP_ME": "value"})

    def test_selected_cli_native_credentials_are_replaced_with_empty_objects(self):
        for kind, paths in [
            ("claude", ["/root/.claude/.credentials.json"]),
            ("codex", ["/root/.codex/auth.json"]),
            ("gemini", ["/root/.gemini/oauth_creds.json"]),
            ("qwen", ["/root/.qwen/oauth_creds.json"]),
            ("opencode", ["/root/.local/share/opencode/auth.json"]),
            ("kilo", ["/root/.local/share/kilo/auth.json"]),
            ("goose", ["/root/.claude/.credentials.json", "/root/.codex/auth.json"])]:
            for auth in ("api", "account"):
                if kind == "qwen" and auth == "account":
                    continue
                spec = {"agent": {"kind": kind, "auth": auth},
                        "agent_endpoint": "http://127.0.0.1:5001/v1", "agent_protocol": "openai"}
                with self.subTest(kind=kind, auth=auth):
                    files = configs(spec, "test-model")
                    for path in paths:
                        self.assertEqual(json.loads(files.get(path, "null")), {})

    def test_credentials_replace_policy_preserves_normal_user_preferences(self):
        self.assertTrue(callable(getattr(agent_auth, "replace_config", None)),
                        "native credential replacement policy is missing")
        for filename in ("/root/.claude/.credentials.json", "/root/.codex/auth.json",
                         "/root/.gemini/oauth_creds.json", "/root/.qwen/oauth_creds.json",
                         "/root/.local/share/opencode/auth.json", "/root/.local/share/kilo/auth.json"):
            self.assertTrue(agent_auth.replace_config(filename), filename)
        for filename in ("/root/.gemini/settings.json", "/root/.config/kilo/kilo.json",
                         "/root/.config/opencode/opencode.json", "/root/work/auth.json"):
            self.assertFalse(agent_auth.replace_config(filename), filename)

    def test_qwen_api_configuration_replaces_legacy_oauth_selection(self):
        for protocol, key in (("openai", "OPENAI_API_KEY"), ("anthropic", "ANTHROPIC_API_KEY")):
            spec = {"agent": {"kind": "qwen", "auth": "api"}, "agent_protocol": protocol,
                    "agent_endpoint": "http://127.0.0.1:5001/v1"}
            config = json.loads(configs(spec, "test-model").get("/root/.qwen/settings.json", "{}"))
            self.assertEqual(config.get("security", {}).get("auth", {}).get("selectedType"), protocol)
            provider = config["modelProviders"][protocol][0]
            self.assertEqual(provider["baseUrl"], "http://127.0.0.1:5001/v1")
            self.assertEqual(provider["envKey"], key)
            self.assertEqual(config["model"]["name"], "test-model")


if __name__ == "__main__":
    unittest.main()
