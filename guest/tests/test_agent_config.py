import importlib.util
import json
import pathlib
import sys
import tomllib
import unittest

GUEST = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(GUEST))


class AgentConfig(unittest.TestCase):
    def files(self, kind, protocol="openai", model="test-model", endpoint="http://127.0.0.1:5001/v1"):
        path = GUEST / "agent_config.py"
        self.assertTrue(path.exists(), "the CLI configuration builder is missing")
        module_spec = importlib.util.spec_from_file_location("agent_config", path)
        module = importlib.util.module_from_spec(module_spec)
        module_spec.loader.exec_module(module)
        return module.configs({"agent": {"kind": kind}, "agent_protocol": protocol,
                               "agent_endpoint": endpoint}, model)

    def test_codex_uses_responses_and_disables_websocket_transport(self):
        config = tomllib.loads(self.files("codex")["/root/.codex/config.toml"])
        provider = config["model_providers"]["agentvm"]
        self.assertEqual(provider["wire_api"], "responses")
        self.assertFalse(provider["supports_websockets"])
        self.assertFalse(provider["requires_openai_auth"])
        self.assertEqual(provider["env_key"], "AGENTVM_DUMMY_API_KEY")
        self.assertEqual(config["projects"]["/root/work"]["trust_level"], "trusted")

    def test_gemini_forces_api_key_auth_instead_of_gateway_auto_detection(self):
        config = json.loads(self.files("gemini", "google")["/root/.gemini/settings.json"])
        self.assertEqual(config["security"]["auth"]["selectedType"], "gemini-api-key")

    def test_opencode_and_kilo_receive_dummy_keys_and_literal_model_ids(self):
        model = "organization/Model:free"
        for kind, path in [("opencode", "/root/.config/opencode/opencode.json"),
                           ("kilo", "/root/.config/kilo/kilo.json")]:
            with self.subTest(kind=kind):
                config = json.loads(self.files(kind, model=model)[path])
                self.assertEqual(config["model"], "agentvm/" + model)
                self.assertEqual(config["provider"]["agentvm"]["options"]["apiKey"], "agentvm-dummy")
                self.assertIn(model, config["provider"]["agentvm"]["models"])
                self.assertEqual(config["permission"], "allow")

    def test_anthropic_sdk_receives_v1_endpoint_once(self):
        config = json.loads(self.files("opencode", "anthropic")["/root/.config/opencode/opencode.json"])
        self.assertEqual(config["provider"]["agentvm"]["npm"], "@ai-sdk/anthropic")
        self.assertEqual(config["provider"]["agentvm"]["options"]["baseURL"], "http://127.0.0.1:5001/v1")

    def test_aider_model_settings_forward_each_protocol_to_the_relay(self):
        for protocol, prefix in [("openai", "openai"), ("anthropic", "anthropic"), ("google", "gemini")]:
            config = json.loads(self.files("aider", protocol)["/run/agentvm/aider-models.json"])
            self.assertEqual(config[0]["name"], prefix + "/test-model")
            base = {"openai": "/v1", "anthropic": "", "google": "/v1/v1beta"}[protocol]
            self.assertEqual(config[0]["extra_params"]["api_base"], "http://127.0.0.1:5001" + base)

    def test_aider_google_base_includes_the_chat_api_version(self):
        config = json.loads(self.files("aider", "google", endpoint="http://127.0.0.1:5001")["/run/agentvm/aider-models.json"])
        self.assertEqual(config[0]["extra_params"]["api_base"], "http://127.0.0.1:5001/v1beta")

    def test_aider_anthropic_does_not_duplicate_v1_in_the_litellm_url(self):
        config = json.loads(self.files("aider", "anthropic")["/run/agentvm/aider-models.json"])
        self.assertEqual(config[0]["extra_params"]["api_base"], "http://127.0.0.1:5001")


if __name__ == "__main__":
    unittest.main()
