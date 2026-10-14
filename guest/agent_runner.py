"""Launch an installed CLI with literal arguments and host-relayed authentication."""
import json
import os
import pathlib
import subprocess
import sys
import threading

from agent_commands import launch
from agent_auth import account_kind, replace_config, sanitize_environment
from agent_config import configs
from agent_events import normalize
from agent_models import discover, needs_model


def run_batch(args, env, prompt, kind, work, job, use_stdin=False, pass_fds=()):
    job = pathlib.Path(job)
    with (job / "agent.log").open("wb") as raw, (job / "agent.err").open("wb") as errors, \
            (job / "stream.jsonl").open("w", encoding="utf-8") as events:
        process = subprocess.Popen(args, env=env, cwd=work, pass_fds=pass_fds,
                                   stdin=subprocess.PIPE if use_stdin else subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=errors)

        def send_prompt():
            try:
                process.stdin.write(prompt.encode("utf-8"))
                process.stdin.close()
            except (BrokenPipeError, OSError):
                pass

        if use_stdin:
            threading.Thread(target=send_prompt, daemon=True).start()
        # Bound individual records even when a CLI writes an unterminated line.
        while line := process.stdout.readline(1024 * 1024):
            raw.write(line)
            raw.flush()
            value = line.decode("utf-8", errors="replace").rstrip("\r\n")
            if not value:
                continue
            try:
                value = json.loads(value)
            except ValueError:
                pass
            for event in normalize(kind, value):
                events.write(json.dumps(event, ensure_ascii=False) + "\n")
            events.flush()
        process.stdout.close()
        status = process.wait()
        return status if status >= 0 else 128 - status


def merge(existing, incoming):
    for key, value in incoming.items():
        if isinstance(value, dict) and isinstance(existing.get(key), dict):
            merge(existing[key], value)
        else:
            existing[key] = value
    return existing


def write_config(filename, content):
    path = pathlib.Path(filename)
    path.parent.mkdir(parents=True, exist_ok=True)
    # Preferences survive restore; native credential caches must be replaced completely.
    if path.suffix == ".json" and path.exists() and not replace_config(str(path)):
        try:
            old, new = json.loads(path.read_text()), json.loads(content)
            if isinstance(old, dict) and isinstance(new, dict):
                content = json.dumps(merge(old, new))
        except (ValueError, OSError):
            pass
    path.write_text(content, encoding="utf-8")
    path.chmod(0o600)


def configure(spec, model):
    for filename, content in configs(spec, model).items():
        write_config(filename, content)


def model_to_prepare(spec, cached):
    if spec.get("model"):
        return spec["model"]
    return None if account_kind(spec) else cached


def prepare_model(spec, runtime):
    cached_model = runtime / "resolved-model"
    model = model_to_prepare(spec, cached_model.read_text() if cached_model.exists() else None)
    if not model and needs_model(spec):
        model = discover(spec)
    if model:
        cached_model.write_text(model)
        # The interactive Claude wrapper reads this shared model file, not task.json.
        (runtime / "model").write_text(model)
    else:
        cached_model.unlink(missing_ok=True)
        (runtime / "model").unlink(missing_ok=True)
    return model


def main():
    job = pathlib.Path("/mnt/job")
    runtime = pathlib.Path("/run/agentvm")
    runtime.mkdir(parents=True, exist_ok=True)
    spec = json.loads((job / "task.json").read_text())
    kind = spec.get("agent", {}).get("kind", "claude")
    model = prepare_model(spec, runtime)
    interactive = spec.get("interactive", False)
    first = not (runtime / "prompt-consumed").exists()
    prompt = spec.get("prompt", "") if first else ""
    restore = bool(spec.get("restore")) and first
    configure(spec, model)
    if "--prepare" in sys.argv[1:]:
        return 0
    args, additions = launch(spec, prompt, model, restore)
    env = sanitize_environment(os.environ)
    env.update({key: value for key, value in additions.items() if value is not None})
    env.update(HOME="/root", USER="root", LANG="C.UTF-8", IS_SANDBOX="1")
    env["PATH"] = "/root/.local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
    (runtime / "prompt-consumed").touch()
    if interactive:
        if kind == "aider" and prompt:
            # Aider's message-file exits after its answer; then reopen the same history.
            (runtime / "prompt").write_text(prompt)
            subprocess.call(args + ["--message-file", str(runtime / "prompt")], env=env, cwd="/root/work")
            if "--restore-chat-history" not in args:
                args.append("--restore-chat-history")
        os.chdir("/root/work")
        os.execvpe(args[0], args, env)
    (runtime / "prompt").write_text(prompt)
    descriptor = None
    if kind == "claude":
        descriptor = os.open(runtime / "token", os.O_RDONLY)
        env["CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR"] = str(descriptor)
        if restore:
            args.append("--continue")
    try:
        return run_batch(args, env, prompt, kind, "/root/work", job,
                         use_stdin=kind in ("claude", "codex"),
                         pass_fds=(descriptor,) if descriptor is not None else ())
    finally:
        if descriptor is not None:
            os.close(descriptor)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, KeyError) as error:
        print(f"Agent launch failed: {error}", file=sys.stderr)
        sys.exit(1)
