"""Select an available provider chat model when a CLI needs one internally."""
import json
import re
import urllib.error
import urllib.request

from agent_auth import account_kind

MAX_CATALOG = 8 * 1024 * 1024
REQUIRED = {"opencode", "qwen", "copilot", "kilo", "aider", "goose"}


def needs_model(spec):
    selection = spec.get("agent") or {}
    kind = selection.get("kind", "claude")
    account = account_kind(spec)
    if account == "chatgpt":
        return True
    if account == "google" or (kind == "goose" and account == "claude"
                                and spec.get("agent_native_defaults") is not False):
        return False
    native = kind in ("claude", "codex", "gemini")
    if kind in REQUIRED or (native and (spec.get("agent_native_defaults") is False
                                        or selection.get("provider") in ("local", "custom", "openrouter"))):
        return True
    custom = selection.get("custom") or {}
    return any("{model}" in (custom.get(key) or "") for key in ("interactive", "headless", "resume"))


def catalog_url(endpoint, protocol):
    endpoint = endpoint.rstrip("/")
    if protocol == "chatgpt":
        return endpoint + "/models?client_version=0.162.1"
    if protocol == "anthropic" and not endpoint.endswith("/v1"):
        endpoint += "/v1"
    if protocol == "google" and not endpoint.endswith(("/v1beta", "/v1alpha")):
        endpoint += "/v1beta"
    return endpoint + "/models"


def select_model(payload, protocol):
    if protocol == "chatgpt":
        candidates = []
        for index, entry in enumerate(payload.get("models") or []):
            if not isinstance(entry, dict) or entry.get("visibility", "list") != "list":
                continue
            name = entry.get("slug")
            if not valid_model(name) or name == "auto" or name.startswith("auto-"):
                continue
            priority = entry.get("priority")
            priority = priority if type(priority) in (int, float) else 1_000_000
            candidates.append(((priority, index), name))
        if not candidates:
            raise ValueError("The account has no available chat model with tool support")
        return min(candidates)[1]
    entries = payload.get("models" if protocol == "google" else "data") or []
    candidates = []
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            continue
        if protocol == "copilot":
            capabilities = entry.get("capabilities") or {}
            if not isinstance(capabilities, dict):
                continue
            supports = capabilities.get("supports") or {}
            if (entry.get("model_picker_enabled") is not True or capabilities.get("type") != "chat"
                    or not isinstance(supports, dict) or supports.get("tool_calls") is not True):
                continue
        name = entry.get("name") if protocol == "google" else entry.get("id")
        if not isinstance(name, str):
            continue
        if protocol == "google":
            name = name.removeprefix("models/")
            if "generateContent" not in entry.get("supportedGenerationMethods", []):
                continue
        if not valid_model(name):
            continue
        lower = name.lower()
        excluded = ("embed", "whisper", "audio", "realtime", "tts", "moderation", "image", "dall-e", "rerank", "text-davinci")
        if any(word in lower for word in excluded) or ("codex" in lower and protocol not in ("copilot", "responses")):
            continue
        if protocol == "copilot" and (name == "auto" or name.startswith("auto-")):
            continue
        architecture = entry.get("architecture") or {}
        if not isinstance(architecture, dict):
            continue
        outputs = architecture.get("output_modalities")
        parameters = entry.get("supported_parameters")
        if (outputs and "text" not in outputs) or (parameters is not None and "tools" not in parameters):
            continue
        score = 0
        if "sonnet" in lower or "gpt-" in lower:
            score += 50
        if "coder" in lower:
            score += 40
        if any(word in lower for word in ("-pro", "preview", "experimental")):
            score -= 5
        created = entry.get("created", 0)
        created = created if isinstance(created, (int, float)) else 0
        candidates.append(((score, created, -index), name))
    if not candidates:
        raise ValueError("The provider has no available chat model with tool support")
    return max(candidates)[1]


def valid_model(name):
    return isinstance(name, str) and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._/:\[\]-]{0,199}", name)


def catalog_protocol(spec):
    account = account_kind(spec)
    if account in ("chatgpt", "copilot"):
        return account
    protocol = spec.get("agent_protocol", "openai")
    if (spec.get("agent") or {}).get("kind") == "codex" and protocol == "openai":
        return "responses"
    return protocol


def catalog_request(spec):
    endpoint = spec.get("agent_endpoint")
    if not endpoint:
        raise ValueError("Automatic model selection needs a configured provider")
    protocol = catalog_protocol(spec)
    headers = {
        "Accept": "application/json", "Authorization": "Bearer agentvm-dummy",
        "anthropic-version": "2023-06-01",
    }
    if protocol == "chatgpt":
        headers.update({"originator": "codex_cli_rs", "User-Agent": "AgentVM"})
    elif protocol == "copilot":
        headers.update({"Copilot-Integration-Id": "copilot-developer-cli",
                        "X-GitHub-Api-Version": "2026-07-01", "User-Agent": "AgentVM"})
    request = urllib.request.Request(catalog_url(endpoint, protocol), headers=headers)
    return request, protocol


def discover(spec):
    request, protocol = catalog_request(spec)
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            data = response.read(MAX_CATALOG + 1)
        if len(data) > MAX_CATALOG:
            raise ValueError("The provider model catalog is too large")
        payload = json.loads(data)
        if not isinstance(payload, dict):
            raise ValueError("The provider returned an invalid model catalog")
        return select_model(payload, protocol)
    except (urllib.error.URLError, OSError, ValueError) as error:
        if isinstance(error, ValueError) and "available chat model" in str(error):
            raise
        raise ValueError("Could not read the provider's available models; check its endpoint and credential") from error
