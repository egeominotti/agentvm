import importlib.util
import json
import pathlib
import sys
import tomllib
import unittest

GUEST = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(GUEST))


def module(name):
    path = GUEST / (name + ".py")
    assert path.exists(), "guest account authentication helper is missing"
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def account_spec(kind, account=None, endpoint="http://127.0.0.1:5001/v1", **extra):
    selection = {"kind": kind, "auth": "account"}
    if account:
        selection["account_kind"] = account
    return {"agent": selection, "agent_endpoint": endpoint,
            "agent_protocol": "openai", "agent_native_defaults": True, **extra}


class AgentAccounts(unittest.TestCase):
    def test_legacy_selections_and_explicit_account_defaults_are_distinct(self):
        auth = module("agent_auth")
        self.assertEqual(auth.account_kind({}), "claude")
        self.assertEqual(auth.account_kind({"agent": {"kind": "claude"}}), "claude")
        self.assertIsNone(auth.account_kind({"agent": {"kind": "claude", "provider": "anthropic"}}))
        self.assertIsNone(auth.account_kind({"agent": {"kind": "codex"}}))
        self.assertIsNone(auth.account_kind({"agent": {"kind": "claude", "auth": "api"}}))
        for kind, wanted in [("claude", "claude"), ("codex", "chatgpt"), ("gemini", "google"),
                             ("copilot", "copilot"), ("opencode", "chatgpt"), ("kilo", "kilo"),
                             ("goose", "claude")]:
            with self.subTest(kind=kind):
                self.assertEqual(auth.account_kind(account_spec(kind)), wanted)
        self.assertEqual(auth.account_kind({"agent": {"kind": "kilo", "account_kind": "chatgpt"}}), "chatgpt")

    def test_invalid_account_selections_do_not_silently_use_api_credentials(self):
        auth = module("agent_auth")
        for selection in [{"kind": "qwen", "auth": "account"},
                          {"kind": "gemini", "auth": "account", "account_kind": "chatgpt"},
                          {"kind": "codex", "auth": "api", "account_kind": "chatgpt"},
                          {"kind": "codex", "auth": "account", "provider": "openai"}]:
            with self.subTest(selection=selection), self.assertRaises(ValueError):
                auth.account_kind({"agent": selection})

    def test_restored_credentials_and_transport_modes_are_removed_from_environment(self):
        auth = module("agent_auth")
        inherited = {"PATH": "/usr/bin", "LANG": "C.UTF-8", "KEEP_ME": "value",
                     "OPENAI_API_KEY": "old-value", "GOOGLE_CLOUD_ACCESS_TOKEN": "old-value",
                     "ANTHROPIC_AUTH_TOKEN": "old-value", "COPILOT_OFFLINE": "true",
                     "COPILOT_PROVIDER_BASE_URL": "https://old.invalid", "KILO_AUTH_CONTENT": "{}",
                     "GOOSE_PROVIDER": "chatgpt_codex", "OPENCODE_CONFIG_CONTENT": "{}"}
        self.assertEqual(auth.sanitize_environment(inherited),
                         {"PATH": "/usr/bin", "LANG": "C.UTF-8", "KEEP_ME": "value"})
        self.assertEqual(inherited["COPILOT_OFFLINE"], "true")

    def test_google_account_uses_code_assist_override_without_api_key_or_oauth_file(self):
        spec = account_spec("gemini", endpoint="http://127.0.0.1:5001")
        env = module("agent_commands").environment(spec, None)
        self.assertEqual(env.get("GOOGLE_GENAI_USE_GCA"), "true")
        self.assertEqual(env.get("GOOGLE_CLOUD_ACCESS_TOKEN"), "agentvm-dummy")
        self.assertEqual(env.get("CODE_ASSIST_ENDPOINT"), "http://127.0.0.1:5001")
        self.assertEqual(env.get("CODE_ASSIST_API_VERSION"), "v1internal")
        for key in ("GEMINI_API_KEY", "GOOGLE_GEMINI_BASE_URL", "GOOGLE_APPLICATION_CREDENTIALS"):
            self.assertNotIn(key, env)
        files = module("agent_config").configs(spec, None)
        settings = json.loads(files["/root/.gemini/settings.json"])
        self.assertEqual(settings["security"]["auth"]["selectedType"], "oauth-personal")
        self.assertEqual(json.loads(files.get("/root/.gemini/oauth_creds.json", "null")), {})

    def test_copilot_account_has_direct_dummy_token_and_no_offline_byok(self):
        spec = account_spec("copilot", endpoint="http://127.0.0.1:5001")
        env = module("agent_commands").environment(spec, "gpt-codex-current")
        self.assertEqual(env.get("COPILOT_API_URL"), "http://127.0.0.1:5001")
        self.assertEqual(env.get("COPILOT_CLI_DISABLE_WEBSOCKET_RESPONSES"), "true")
        self.assertEqual(env.get("GITHUB_COPILOT_API_TOKEN"), "agentvm-dummy")
        self.assertEqual(env.get("COPILOT_MODEL"), "gpt-codex-current")
        for key in ("COPILOT_OFFLINE", "COPILOT_PROVIDER_TYPE", "COPILOT_PROVIDER_BASE_URL",
                    "COPILOT_PROVIDER_API_KEY", "GITHUB_TOKEN", "GH_TOKEN", "COPILOT_API_TOKEN"):
            self.assertNotIn(key, env)

    def test_chatgpt_uses_custom_responses_provider_without_native_oauth_plugins(self):
        for kind in ("opencode", "kilo"):
            spec = account_spec(kind, "chatgpt", "http://127.0.0.1:5001/backend-api/codex")
            config = json.loads(module("agent_config").configs(spec, "gpt-6-codex")
                                [f"/root/.config/{kind}/{kind}.json"])
            with self.subTest(kind=kind):
                self.assertEqual(config["model"], "agentvm/gpt-6-codex")
                self.assertEqual(config.get("enabled_providers"), ["agentvm"])
                provider = config["provider"]["agentvm"]
                self.assertEqual(provider["npm"], "@ai-sdk/openai")
                self.assertEqual(provider["options"]["baseURL"], "http://127.0.0.1:5001/backend-api/codex")
                self.assertEqual(provider["options"]["apiKey"], "agentvm-dummy")
                self.assertNotIn("openai", config["provider"])

    def test_kilo_account_gateway_uses_its_exact_relay_path(self):
        spec = account_spec("kilo", "kilo", "http://127.0.0.1:5001/api/openrouter")
        config = json.loads(module("agent_config").configs(spec, "organization/model")
                            ["/root/.config/kilo/kilo.json"])
        provider = config["provider"]["agentvm"]
        self.assertEqual(provider["npm"], "@ai-sdk/openai-compatible")
        self.assertEqual(provider["options"]["baseURL"], "http://127.0.0.1:5001/api/openrouter")
        self.assertEqual(config.get("enabled_providers"), ["agentvm"])
        env = module("agent_commands").environment(spec, "organization/model")
        self.assertEqual(env.get("KILO_API_URL"), "http://127.0.0.1:5001")
        self.assertEqual(env.get("KILO_API_KEY"), "agentvm-dummy")
        self.assertEqual(env.get("KILO_AUTH_CONTENT"), "{}")

    def test_codex_chatgpt_sets_the_discovered_model_and_uses_no_native_auth_file(self):
        spec = account_spec("codex", endpoint="http://127.0.0.1:5001/backend-api/codex")
        files = module("agent_config").configs(spec, "gpt-6-codex")
        config = tomllib.loads(files["/root/.codex/config.toml"])
        self.assertEqual(config.get("model"), "gpt-6-codex")
        self.assertEqual(config["model_providers"]["agentvm"]["base_url"],
                         "http://127.0.0.1:5001/backend-api/codex")
        self.assertFalse(config["model_providers"]["agentvm"]["requires_openai_auth"])
        self.assertEqual(json.loads(files.get("/root/.codex/auth.json", "null")), {})

    def test_goose_chatgpt_uses_pinned_codex_cli_and_relay_configuration(self):
        spec = account_spec("goose", "chatgpt", "http://127.0.0.1:5001/backend-api/codex",
                            interactive=False)
        prompt = "--help; $(touch /tmp/owned)\n'quoted'"
        args, env = module("agent_commands").launch(spec, prompt, "gpt-6-codex", True)
        self.assertIn("--text=" + prompt, args)
        self.assertEqual(args[args.index("--provider") + 1], "codex")
        self.assertEqual(env.get("GOOSE_PROVIDER"), "codex")
        self.assertEqual(env.get("CODEX_COMMAND"), "codex")
        self.assertEqual(env.get("GOOSE_MODEL"), "gpt-6-codex")
        files = module("agent_config").configs(spec, "gpt-6-codex")
        config = tomllib.loads(files["/root/.codex/config.toml"])
        self.assertEqual(config.get("model"), "gpt-6-codex")
        self.assertEqual(config["model_providers"]["agentvm"]["base_url"],
                         "http://127.0.0.1:5001/backend-api/codex")

    def test_goose_claude_uses_dummy_oauth_and_native_default_model(self):
        spec = account_spec("goose", endpoint="http://127.0.0.1:5001")
        try:
            args, env = module("agent_commands").launch(spec, "hello", None, False)
        except ValueError as error:
            self.fail(f"Claude account should use its native default model: {error}")
        self.assertEqual(args[args.index("--provider") + 1], "claude-code")
        self.assertEqual(args[args.index("--model") + 1], "default")
        self.assertEqual(env.get("CLAUDE_CODE_COMMAND"), "claude")
        self.assertEqual(env.get("CLAUDE_CODE_OAUTH_TOKEN"), "agentvm-dummy")
        self.assertEqual(env.get("ANTHROPIC_BASE_URL"), "http://127.0.0.1:5001")
        self.assertNotIn("ANTHROPIC_API_KEY", env)
        self.assertNotIn("OPENAI_API_KEY", env)

    def test_api_mode_does_not_fall_back_to_a_claude_account(self):
        spec = {"agent": {"kind": "claude", "auth": "api"},
                "agent_endpoint": "http://127.0.0.1:5001", "agent_protocol": "anthropic"}
        env = module("agent_commands").environment(spec, None)
        self.assertEqual(env.get("ANTHROPIC_AUTH_TOKEN"), "agentvm-dummy")
        self.assertNotIn("CLAUDE_CODE_OAUTH_TOKEN", env)


class AccountModels(unittest.TestCase):
    def test_chatgpt_catalog_uses_visible_slugs_and_priority_instead_of_api_data(self):
        payload = {"data": [{"id": "ordinary-api-model"}], "models": [
            {"slug": "hidden-codex", "visibility": "hidden", "priority": 1},
            {"slug": "auto", "visibility": "list", "priority": 0},
            {"slug": "gpt-new-codex", "visibility": "list", "priority": 2},
            {"slug": "gpt-old-codex", "visibility": "list", "priority": 10}]}
        self.assertEqual(module("agent_models").select_model(payload, "chatgpt"), "gpt-new-codex")

    def test_chatgpt_catalog_rejects_hidden_auto_and_invalid_models(self):
        for models in [[], [{"slug": "auto", "visibility": "list"}],
                       [{"slug": "hidden", "visibility": "hidden"}],
                       [{"slug": "bad\nmodel", "visibility": "list"}]]:
            with self.subTest(models=models), self.assertRaisesRegex(ValueError, "available chat model"):
                module("agent_models").select_model({"models": models}, "chatgpt")

    def test_chatgpt_model_discovery_is_required_even_for_native_defaults(self):
        models = module("agent_models")
        for kind in ("codex", "opencode", "kilo", "goose"):
            self.assertTrue(models.needs_model(account_spec(kind, "chatgpt")))
        self.assertFalse(models.needs_model(account_spec("gemini", "google")))
        self.assertFalse(models.needs_model(account_spec("goose", "claude")))

    def test_chatgpt_catalog_url_preserves_backend_base_and_cli_version_query(self):
        self.assertEqual(module("agent_models").catalog_url(
            "http://127.0.0.1:5001/backend-api/codex", "chatgpt"),
            "http://127.0.0.1:5001/backend-api/codex/models?client_version=0.162.1")

    def test_copilot_catalog_requires_visible_chat_models_with_tools(self):
        payload = {"data": [
            {"id": "hidden-gpt", "model_picker_enabled": False,
             "capabilities": {"type": "chat", "supports": {"tool_calls": True}}},
            {"id": "embed", "model_picker_enabled": True, "capabilities": {"type": "embeddings"}},
            {"id": "gpt-6-codex", "model_picker_enabled": True,
             "capabilities": {"type": "chat", "supports": {"tool_calls": True}}},
            {"id": "gpt-no-tools", "model_picker_enabled": True,
             "capabilities": {"type": "chat", "supports": {"tool_calls": False}}}]}
        self.assertEqual(module("agent_models").select_model(payload, "copilot"), "gpt-6-codex")

    def test_catalog_requests_bind_account_schema_and_dummy_auth_to_the_relay(self):
        models = module("agent_models")
        self.assertTrue(callable(getattr(models, "catalog_request", None)),
                        "account catalog request builder is missing")
        for spec, expected_url, schema in [
            (account_spec("codex", endpoint="http://127.0.0.1:5001/backend-api/codex"),
             "http://127.0.0.1:5001/backend-api/codex/models?client_version=0.162.1", "chatgpt"),
            (account_spec("copilot", endpoint="http://127.0.0.1:5001"),
             "http://127.0.0.1:5001/models", "copilot"),
            ({"agent": {"kind": "codex", "auth": "api"},
              "agent_endpoint": "http://127.0.0.1:5001/v1", "agent_protocol": "openai"},
             "http://127.0.0.1:5001/v1/models", "responses")]:
            with self.subTest(schema=schema):
                request, protocol = models.catalog_request(spec)
                self.assertEqual(request.full_url, expected_url)
                self.assertEqual(request.get_header("Authorization"), "Bearer agentvm-dummy")
                self.assertEqual(protocol, schema)

    def test_chatgpt_catalog_headers_identify_agentvm_and_the_codex_client_contract(self):
        request, _ = module("agent_models").catalog_request(account_spec(
            "codex", endpoint="http://127.0.0.1:5001/backend-api/codex"))
        headers = {key.lower(): value for key, value in request.header_items()}
        self.assertEqual(headers.get("originator"), "codex_cli_rs")
        self.assertEqual(headers.get("user-agent"), "AgentVM")

    def test_copilot_catalog_headers_use_its_documented_integration_and_honest_client_name(self):
        request, protocol = module("agent_models").catalog_request(account_spec(
            "copilot", endpoint="http://127.0.0.1:5001"))
        headers = {key.lower(): value for key, value in request.header_items()}
        self.assertEqual(protocol, "copilot")
        self.assertEqual(request.full_url, "http://127.0.0.1:5001/models")
        self.assertEqual(headers.get("copilot-integration-id"), "copilot-developer-cli")
        self.assertEqual(headers.get("x-github-api-version"), "2026-07-01")
        self.assertEqual(headers.get("user-agent"), "AgentVM")
        self.assertNotIn("originator", headers)


if __name__ == "__main__":
    unittest.main()
