"""Build CLI arguments without evaluating prompts or model IDs as shell code."""
import shlex
from urllib.parse import urlsplit

from agent_auth import DUMMY, account_environment, account_kind, goose_provider

PROMPT_FILE = "/run/agentvm/prompt"


def environment(spec, model):
    endpoint = spec.get("agent_endpoint")
    if not endpoint:
        return {}
    account_env = account_environment(spec, model)
    if account_env is not None:
        return account_env
    protocol = spec.get("agent_protocol", "openai")
    env = {"AGENTVM_DUMMY_API_KEY": DUMMY, "OPENAI_API_KEY": DUMMY,
           "OPENAI_BASE_URL": endpoint, "OPENAI_API_BASE": endpoint,
           "ANTHROPIC_API_KEY": DUMMY, "ANTHROPIC_BASE_URL": endpoint,
           "ANTHROPIC_API_BASE": endpoint, "GEMINI_API_KEY": DUMMY,
           "GOOGLE_GEMINI_BASE_URL": endpoint, "GEMINI_API_BASE": endpoint}
    kind = spec.get("agent", {}).get("kind", "claude")
    if kind == "claude":
        env.pop("ANTHROPIC_API_KEY")
        if spec.get("agent", {}).get("provider") or spec.get("agent", {}).get("auth") == "api":
            env["ANTHROPIC_AUTH_TOKEN"] = DUMMY
    elif kind == "gemini":
        env.update(GEMINI_SANDBOX="false", GEMINI_CLI_TRUST_WORKSPACE="true")
    elif kind == "qwen":
        # Qwen auto-detects authentication from environment; expose one protocol only.
        if protocol == "anthropic":
            env.pop("OPENAI_API_KEY")
            env["ANTHROPIC_MODEL"] = model
        else:
            env.pop("ANTHROPIC_API_KEY")
            env["OPENAI_MODEL"] = model
    elif kind == "copilot":
        env.update(COPILOT_OFFLINE="true", COPILOT_PROVIDER_TYPE=protocol,
                   COPILOT_PROVIDER_BASE_URL=endpoint, COPILOT_PROVIDER_API_KEY=DUMMY)
        if model:
            env["COPILOT_MODEL"] = model
    elif kind == "goose":
        parsed = urlsplit(endpoint)
        env.update(GOOSE_MODE="auto", GOOSE_PROVIDER="openai", GOOSE_MODEL=model,
                   OPENAI_HOST=f"{parsed.scheme}://{parsed.netloc}",
                   OPENAI_BASE_PATH=parsed.path.strip("/") + "/chat/completions")
    return env


def custom_command(spec, prompt, model, restore):
    custom = spec.get("agent", {}).get("custom") or {}
    key = "interactive" if spec.get("interactive") else "headless"
    if restore and key == "interactive" and custom.get("resume"):
        key = "resume"
    template = custom.get(key)
    if not template:
        raise ValueError(f"custom CLI needs a {key} command")
    values = {"{prompt}": prompt, "{model}": model or "",
              "{endpoint}": spec.get("agent_endpoint") or ""}
    args = shlex.split(template)
    if not args:
        raise ValueError("custom CLI command is empty")
    # Each template token is substituted once, so a prompt containing {model} stays literal.
    import re
    return [re.sub(r"\{(?:prompt|model|endpoint)\}", lambda m: values[m.group()], arg) for arg in args]


def launch(spec, prompt, model, restore):
    kind = spec.get("agent", {}).get("kind", "claude")
    if kind == "goose" and account_kind(spec) == "claude":
        model = model or "default"
    if kind in ("opencode", "qwen", "copilot", "kilo", "aider", "goose") and not model:
        raise ValueError(f"{kind} requires a model")
    interactive = spec.get("interactive", False)
    env = environment(spec, model)
    model_args = ["--model", model] if model else []
    if kind == "claude":
        if interactive:
            return ["/usr/local/bin/agentvm-claude"], env
        return ["/root/.local/bin/claude", "-p", "--dangerously-skip-permissions",
                *model_args, "--output-format", "stream-json", "--verbose"], env
    if kind == "codex":
        args = ["codex"]
        if not interactive:
            args += ["exec"]
            if restore:
                args += ["resume", "--last"]
        elif restore:
            args += ["resume", "--last"]
        args += ["--dangerously-bypass-approvals-and-sandbox", *model_args]
        if not interactive:
            args += ["--json", "-"]
        elif prompt:
            args += ["--", prompt]
    elif kind == "gemini":
        args = ["gemini", "--approval-mode=yolo", *model_args]
        if restore:
            args += ["--resume", "latest"]
        if prompt or not interactive:
            args += [("--prompt-interactive=" if interactive else "--prompt=") + prompt]
        if not interactive:
            args += ["--output-format", "stream-json"]
    elif kind == "qwen":
        args = ["qwen", "--yolo", *model_args]
        if restore:
            args += ["--continue"]
        if prompt or not interactive:
            args += [("--prompt-interactive=" if interactive else "--prompt=") + prompt]
        if not interactive:
            args += ["--output-format", "stream-json"]
    elif kind == "copilot":
        args = ["copilot", "--allow-all", "--no-ask-user", *model_args]
        if restore:
            args += ["--continue"]
        if prompt or not interactive:
            args += [("--interactive=" if interactive else "--prompt=") + prompt]
        if not interactive:
            args += ["--output-format", "json"]
    elif kind in ("opencode", "kilo"):
        args = [kind] if interactive else [kind, "run"]
        if kind == "kilo":
            args += ["--auto"]
        args += ["--model", "agentvm/" + model]
        if restore:
            args += ["--continue"]
        if interactive and prompt:
            args += ["--prompt=" + prompt]
        elif not interactive:
            args += ["--format", "json", "--", prompt]
    elif kind == "aider":
        prefix = {"openai": "openai", "anthropic": "anthropic", "google": "gemini"}[spec.get("agent_protocol", "openai")]
        args = ["aider", "--model", prefix + "/" + model, "--yes-always", "--no-auto-commits",
                "--model-settings-file", "/run/agentvm/aider-models.json"]
        if restore:
            args += ["--restore-chat-history"]
        if not interactive:
            args += ["--message-file", PROMPT_FILE]
    elif kind == "goose":
        args = ["goose", "run", "--text=" + prompt] if prompt or not interactive else ["goose", "session"]
        if interactive and prompt:
            args += ["-s"]
        args += ["--name", "agentvm", "--with-builtin", "developer"]
        if restore:
            args += ["--resume"]
        if args[1] == "run":
            args += ["--provider", goose_provider(spec), *model_args]
        if not interactive:
            args += ["--output-format", "stream-json"]
    elif kind == "custom":
        args = custom_command(spec, prompt, model, restore)
    else:
        raise ValueError(f"unsupported CLI: {kind}")
    return args, env
