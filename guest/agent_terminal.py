"""Literal tmux operations; disconnecting the control client leaves sessions alive."""
import os
import subprocess
from agent_exec import integer

SESSIONS = {"claude", "shell", *(f"shell-{n}" for n in range(2, 10))}
KEYS = {"Enter", "Escape", "Tab", "BTab", "BSpace", "Up", "Down", "Left", "Right", "Home", "End", "PPage", "NPage", "DC", *(f"C-{c}" for c in "abcdefghijklmnopqrstuvwxyz")}


def tmux(*args):
    try:
        result = subprocess.run(["tmux", "-f", "/etc/agentvm/tmux.conf", *args], capture_output=True, timeout=10)
    except subprocess.TimeoutExpired as error:
        raise ValueError("terminal operation timed out") from error
    if result.returncode:
        raise ValueError(result.stderr.decode(errors="replace")[:4096] or "tmux operation failed")
    return result.stdout[:1024 * 1024].decode(errors="replace")


def missing_server(message):
    message = message.strip()
    if "\n" in message:
        return False
    return (message == "no sessions" or message.startswith("no server running on ")
            or (message.startswith("error connecting to ")
                and message.endswith(" (No such file or directory)")))


def dispatch(op, args):
    if op == "list":
        try:
            output = tmux("list-sessions", "-F", "#{session_name}")
        except ValueError as error:
            if missing_server(str(error)):
                output = ""
            else:
                raise
        return {"sessions": [name for name in output.splitlines() if name in SESSIONS]}
    session = args.get("session")
    if session not in SESSIONS:
        raise ValueError("unsupported terminal session")
    # Pane/window commands need the session separator; a bare name is a window target.
    target = "=" + session + ":"
    if op == "create":
        if session == "claude":
            raise ValueError("the agent session is managed by the VM runtime")
        shell = "/bin/zsh" if os.path.exists("/bin/zsh") else "/bin/bash"
        tmux("new-session", "-d", "-A", "-s", session, "-c", args.get("cwd", "/root/work"), shell, "-l")
        return {"session": session}
    if op == "capture":
        lines = integer(args, "lines", 200, 1, 2000)
        return {"text": tmux("capture-pane", "-p", "-t", target, "-S", str(-lines))}
    if op == "input":
        text = args.get("text", "")
        keys = args.get("keys", [])
        if not isinstance(text, str) or len(text.encode()) > 65536:
            raise ValueError("text must be at most 64 KiB")
        if not isinstance(keys, list) or len(keys) > 100 or any(key not in KEYS for key in keys):
            raise ValueError("unsupported terminal keys")
        if text:
            tmux("send-keys", "-t", target, "-l", "--", text)
        if keys:
            tmux("send-keys", "-t", target, "--", *keys)
        return {"ok": True}
    if op == "resize":
        cols = integer(args, "cols", 120, 1, 500)
        rows = integer(args, "rows", 36, 1, 500)
        tmux("resize-window", "-t", target, "-x", str(cols), "-y", str(rows))
        return {"cols": cols, "rows": rows}
    raise ValueError("unsupported terminal operation")
