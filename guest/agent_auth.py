"""Resolve account choices and keep inherited authentication out of guest launches."""
from urllib.parse import urlsplit

DUMMY = "agentvm-dummy"
ACCOUNTS = {"claude": ("claude",), "codex": ("chatgpt",), "gemini": ("google",),
            "copilot": ("copilot",), "opencode": ("chatgpt",),
            "kilo": ("kilo", "chatgpt"), "goose": ("claude", "chatgpt")}
AUTH_PATHS = {"claude": (".claude/.credentials.json",), "codex": (".codex/auth.json",),
              "gemini": (".gemini/oauth_creds.json",), "qwen": (".qwen/oauth_creds.json",),
              "opencode": (".local/share/opencode/auth.json",),
              "kilo": (".local/share/kilo/auth.json",),
              "goose": (".claude/.credentials.json", ".codex/auth.json")}
AUTH_ENV = frozenset("""
    AGENTVM_DUMMY_API_KEY OPENAI_API_KEY OPENAI_BASE_URL OPENAI_API_BASE OPENAI_HOST
    OPENAI_BASE_PATH OPENAI_MODEL OPENAI_ORG_ID OPENAI_ORGANIZATION CHATGPT_ACCOUNT_ID
    ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN ANTHROPIC_BASE_URL ANTHROPIC_API_BASE
    ANTHROPIC_MODEL CLAUDE_CODE_OAUTH_TOKEN CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR
    CLAUDE_CODE_API_KEY_HELPER CLAUDE_CODE_COMMAND GEMINI_API_KEY GEMINI_MODEL
    GOOGLE_API_KEY GOOGLE_GEMINI_BASE_URL GEMINI_API_BASE GOOGLE_GENAI_USE_GCA
    GOOGLE_CLOUD_ACCESS_TOKEN GOOGLE_GENAI_USE_VERTEXAI GOOGLE_APPLICATION_CREDENTIALS
    CODE_ASSIST_ENDPOINT CODE_ASSIST_API_VERSION COPILOT_API_TOKEN GITHUB_COPILOT_API_TOKEN
    COPILOT_API_URL
    COPILOT_OFFLINE COPILOT_PROVIDER_TYPE COPILOT_PROVIDER_BASE_URL
    COPILOT_PROVIDER_API_KEY COPILOT_MODEL COPILOT_CLI_DISABLE_WEBSOCKET_RESPONSES
    GH_TOKEN GITHUB_TOKEN COPILOT_GITHUB_TOKEN KILO_API_KEY KILO_API_URL KILO_AUTH_CONTENT
    KILO_ORG_ID KILO_CONFIG KILO_CONFIG_CONTENT OPENCODE_AUTH_CONTENT OPENCODE_CONFIG
    OPENCODE_CONFIG_CONTENT OPENCODE_CONSOLE_TOKEN GOOSE_PROVIDER GOOSE_MODEL CODEX_COMMAND
    GOOSE_MODE
""".split())


def account_kind(spec):
    selection = spec.get("agent") or {}
    kind, auth, account = (selection.get(key) for key in ("kind", "auth", "account_kind"))
    kind = kind or "claude"
    if auth not in (None, "account", "api"):
        raise ValueError("Unsupported authentication mode")
    if auth == "api":
        if account:
            raise ValueError("API authentication cannot select an account")
        return None
    if auth == "account" or account:
        eligible = ACCOUNTS.get(kind, ())
        if selection.get("provider") or not eligible:
            raise ValueError(f"{kind} cannot use the selected account authentication")
        account = account or eligible[0]
        if account not in eligible:
            raise ValueError(f"{kind} cannot use a {account} account")
        return account
    # Old task metadata only represented Claude subscription access without a provider.
    return "claude" if kind == "claude" and not selection.get("provider") else None


def sanitize_environment(environ):
    return {key: value for key, value in environ.items() if key not in AUTH_ENV}


def replace_config(filename):
    return any(str(filename).endswith("/" + path) for paths in AUTH_PATHS.values() for path in paths)


def credential_configs(spec):
    kind = (spec.get("agent") or {}).get("kind", "claude")
    return {"/root/" + path: "{}" for path in AUTH_PATHS.get(kind, ())}


def goose_provider(spec):
    return {"claude": "claude-code", "chatgpt": "codex"}.get(account_kind(spec), "openai")


def install_dependencies(spec):
    if (spec.get("agent") or {}).get("kind") == "goose" and account_kind(spec) == "chatgpt":
        return [{"kind": "codex", "package": "@openai/codex", "version": "0.162.1"}]
    return []


def account_environment(spec, model):
    account = account_kind(spec)
    if not account:
        return None
    endpoint = (spec.get("agent_endpoint") or "").rstrip("/")
    if not endpoint:
        return {}
    kind = (spec.get("agent") or {}).get("kind", "claude")
    env = {"AGENTVM_DUMMY_API_KEY": DUMMY}
    if account == "claude":
        env.update(ANTHROPIC_BASE_URL=endpoint, ANTHROPIC_API_BASE=endpoint)
        if kind == "goose":
            env.update(CLAUDE_CODE_OAUTH_TOKEN=DUMMY, CLAUDE_CODE_COMMAND="claude",
                       GOOSE_MODE="auto", GOOSE_PROVIDER="claude-code", GOOSE_MODEL=model or "default")
    elif account == "google":
        env.update(GOOGLE_GENAI_USE_GCA="true", GOOGLE_CLOUD_ACCESS_TOKEN=DUMMY,
                   CODE_ASSIST_ENDPOINT=endpoint, CODE_ASSIST_API_VERSION="v1internal",
                   GEMINI_SANDBOX="false", GEMINI_CLI_TRUST_WORKSPACE="true")
    elif account == "copilot":
        env.update(GITHUB_COPILOT_API_TOKEN=DUMMY, COPILOT_API_URL=endpoint,
                   COPILOT_CLI_DISABLE_WEBSOCKET_RESPONSES="true", COPILOT_MODEL=model)
    else:
        env.update(OPENAI_API_KEY=DUMMY, OPENAI_BASE_URL=endpoint, OPENAI_API_BASE=endpoint)
        if kind == "goose":
            env.update(GOOSE_MODE="auto", GOOSE_PROVIDER="codex", GOOSE_MODEL=model,
                       CODEX_COMMAND="codex")
        if kind == "opencode":
            env["OPENCODE_AUTH_CONTENT"] = "{}"
        if kind == "kilo":
            parsed = urlsplit(endpoint)
            env.update(KILO_API_URL=f"{parsed.scheme}://{parsed.netloc}",
                       KILO_API_KEY=DUMMY, KILO_AUTH_CONTENT="{}")
    return env
