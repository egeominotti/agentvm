"""Non-secret CLI configuration; provider credentials remain on the host."""
import json

from agent_auth import DUMMY, account_kind, credential_configs


def codex_config(endpoint, model):
    lines = ['model_provider = "agentvm"', 'check_for_update_on_startup = false']
    if model:
        lines.append('model = ' + json.dumps(model))
    lines += ['[model_providers.agentvm]', 'name = "AgentVM"',
              'base_url = ' + json.dumps(endpoint), 'env_key = "AGENTVM_DUMMY_API_KEY"',
              'wire_api = "responses"', 'requires_openai_auth = false', 'supports_websockets = false',
              '[projects."/root/work"]', 'trust_level = "trusted"', '']
    return {"/root/.codex/config.toml": '\n'.join(lines)}


def provider_configs(spec, model):
    kind = spec.get("agent", {}).get("kind", "claude")
    endpoint = (spec.get("agent_endpoint") or "").rstrip("/")
    protocol = spec.get("agent_protocol", "openai")
    account = account_kind(spec)
    if not endpoint:
        return {}
    if kind == "codex" or (kind == "goose" and account == "chatgpt"):
        return codex_config(endpoint, model)
    if kind == "gemini":
        auth = "oauth-personal" if account == "google" else "gemini-api-key"
        return {"/root/.gemini/settings.json": json.dumps({"security": {"auth": {"selectedType": auth}}})}
    if kind == "qwen":
        auth = "gemini" if protocol == "google" else protocol
        key = {"openai": "OPENAI_API_KEY", "anthropic": "ANTHROPIC_API_KEY", "google": "GEMINI_API_KEY"}[protocol]
        config = {"security": {"auth": {"selectedType": auth}}, "model": {"name": model},
                  "modelProviders": {auth: [{"id": model, "baseUrl": endpoint, "envKey": key}]}}
        return {"/root/.qwen/settings.json": json.dumps(config)}
    if kind in ("opencode", "kilo"):
        npm = {"openai": "@ai-sdk/openai-compatible", "anthropic": "@ai-sdk/anthropic",
               "google": "@ai-sdk/google"}[protocol]
        if account == "chatgpt":
            npm = "@ai-sdk/openai"
        sdk_endpoint = endpoint
        if protocol == "anthropic" and not endpoint.endswith("/v1"):
            sdk_endpoint += "/v1"
        if protocol == "google" and not endpoint.endswith("/v1beta"):
            sdk_endpoint += "/v1beta"
        config = {"model": "agentvm/" + model, "permission": "allow",
                  "provider": {"agentvm": {"npm": npm, "name": "AgentVM",
                               "options": {"baseURL": sdk_endpoint, "apiKey": DUMMY},
                               "models": {model: {"name": model}}}}}
        if account:
            # Built-in OAuth plugins can rewrite requests to an unrelayed upstream.
            config["enabled_providers"] = ["agentvm"]
        return {f"/root/.config/{kind}/{kind}.json": json.dumps(config)}
    if kind == "aider":
        prefix = {"openai": "openai", "anthropic": "anthropic", "google": "gemini"}[protocol]
        if protocol == "google" and not endpoint.endswith(("/v1beta", "/v1alpha")):
            endpoint += "/v1beta"
        if protocol == "anthropic" and endpoint.endswith("/v1"):
            endpoint = endpoint[:-3]
        config = [{"name": prefix + "/" + model, "extra_params": {"api_base": endpoint}}]
        return {"/run/agentvm/aider-models.json": json.dumps(config)}
    return {}


def configs(spec, model):
    return {**credential_configs(spec), **provider_configs(spec, model)}
