import importlib.util
import pathlib
import sys
import tomllib
import unittest

GUEST = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(GUEST))


class AgentModels(unittest.TestCase):
    def module(self):
        path = GUEST / "agent_models.py"
        self.assertTrue(path.exists(), "automatic model selection is missing")
        spec = importlib.util.spec_from_file_location("agent_models", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_openai_catalog_selects_current_chat_model_without_audio_or_responses_only(self):
        payload = {"data": [{"id": "whisper-1", "created": 100},
                            {"id": "gpt-4.1", "created": 10},
                            {"id": "gpt-5.4-codex", "created": 100},
                            {"id": "gpt-5.4", "created": 50}]}
        self.assertEqual(self.module().select_model(payload, "openai"), "gpt-5.4")

    def test_responses_catalog_retains_codex_models_for_the_configured_wire_api(self):
        payload = {"data": [{"id": "gpt-5.4", "created": 50},
                            {"id": "gpt-5.4-codex", "created": 100}]}
        self.assertEqual(self.module().select_model(payload, "responses"), "gpt-5.4-codex")

    def test_codex_api_catalog_selection_matches_its_responses_wire_contract(self):
        from agent_config import configs
        module = self.module()
        for provider in ("local", "custom", "opencodego", "opencodezen"):
            spec = {"agent": {"kind": "codex", "auth": "api", "provider": provider},
                    "agent_endpoint": "http://127.0.0.1:5001/v1", "agent_protocol": "openai",
                    "agent_native_defaults": False}
            with self.subTest(provider=provider):
                config = tomllib.loads(configs(spec, "gpt-5.4-codex")["/root/.codex/config.toml"])
                self.assertEqual(config["model_providers"]["agentvm"]["wire_api"], "responses")
                self.assertTrue(module.needs_model(spec))
                request, protocol = module.catalog_request(spec)
                self.assertEqual(protocol, "responses")
                self.assertEqual(request.full_url, "http://127.0.0.1:5001/v1/models")

    def test_chat_only_cli_catalog_selection_still_excludes_codex_models(self):
        module = self.module()
        payload = {"data": [{"id": "gpt-5.4", "created": 50},
                            {"id": "gpt-5.4-codex", "created": 100}]}
        for kind in ("opencode", "qwen", "copilot", "kilo", "aider", "goose"):
            spec = {"agent": {"kind": kind, "auth": "api", "provider": "opencodego"},
                    "agent_protocol": "openai"}
            with self.subTest(kind=kind):
                protocol = module.catalog_protocol(spec)
                self.assertEqual(protocol, "openai")
                self.assertEqual(module.select_model(payload, protocol), "gpt-5.4")

    def test_local_catalog_uses_an_installed_chat_model(self):
        payload = {"data": [{"id": "nomic-embed-text"}, {"id": "qwen3-coder:latest"}]}
        self.assertEqual(self.module().select_model(payload, "openai"), "qwen3-coder:latest")

    def test_google_catalog_requires_generate_content_and_strips_resource_prefix(self):
        payload = {"models": [{"name": "models/embedding-001", "supportedGenerationMethods": ["embedContent"]},
                              {"name": "models/gemini-3-pro", "supportedGenerationMethods": ["generateContent"]}]}
        self.assertEqual(self.module().select_model(payload, "google"), "gemini-3-pro")

    def test_anthropic_catalog_prefers_sonnet_over_opus(self):
        payload = {"data": [{"id": "claude-opus-current"}, {"id": "claude-sonnet-current"}]}
        self.assertEqual(self.module().select_model(payload, "anthropic"), "claude-sonnet-current")

    def test_openrouter_filters_models_without_text_output_or_tools(self):
        payload = {"data": [{"id": "image/model", "architecture": {"output_modalities": ["image"]}},
                            {"id": "text/no-tools", "supported_parameters": ["temperature"]},
                            {"id": "openai/gpt-current", "supported_parameters": ["tools"]}]}
        self.assertEqual(self.module().select_model(payload, "openai"), "openai/gpt-current")

    def test_empty_catalog_reports_provider_problem_instead_of_requesting_an_id(self):
        with self.assertRaisesRegex(ValueError, "available chat model"):
            self.module().select_model({"data": []}, "openai")

    def test_model_urls_keep_protocol_versions_once(self):
        module = self.module()
        self.assertEqual(module.catalog_url("http://127.0.0.1:5001/v1", "openai"), "http://127.0.0.1:5001/v1/models")
        self.assertEqual(module.catalog_url("http://127.0.0.1:5001/v1", "anthropic"), "http://127.0.0.1:5001/v1/models")
        self.assertEqual(module.catalog_url("http://127.0.0.1:5001", "google"), "http://127.0.0.1:5001/v1beta/models")

    def test_local_codex_and_custom_gemini_use_provider_catalogs(self):
        module = self.module()
        self.assertTrue(module.needs_model({"agent": {"kind": "codex", "provider": "local"}}))
        self.assertTrue(module.needs_model({"agent": {"kind": "gemini", "provider": "custom"}}))
        self.assertFalse(module.needs_model({"agent": {"kind": "codex"}}))
        self.assertFalse(module.needs_model({"agent": {"kind": "claude"}}))

    def test_overridden_native_provider_endpoints_use_the_available_model_catalog(self):
        module = self.module()
        for kind, provider in (("codex", "openai"), ("gemini", "google"), ("claude", "anthropic")):
            with self.subTest(kind=kind):
                self.assertTrue(module.needs_model({"agent": {"kind": kind, "provider": provider},
                                                   "agent_native_defaults": False}))


if __name__ == "__main__":
    unittest.main()
