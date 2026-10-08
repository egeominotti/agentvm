# agentvm — Terminals (every terminal is a VM)

Date: 2026-10-08 · Status: approved in chat · Extends `2026-10-08-agentvm-mvp-design.md`

## Goal

Every terminal opened in the dashboard is a dedicated Debian VM containing the chosen repo and
**interactive Claude Code** (the real TUI, with `--dangerously-skip-permissions`). The user writes
directly to the agent, opens shells in the same VM, saves the work as an `agent/<id>` branch and
closes the VM. "One agent per line" opens N terminals, each with Claude started on that task.

## Decisions

- **Terminal channel: vsock**, not network. The Swift helper adds a `VZVirtioSocketDevice` and
  listens on a Unix socket (`jobs/<id>/pty.sock`); each connection is forwarded to the guest's
  vsock port 5000. The Rust server bridges WebSocket ↔ Unix socket.
- **PTY server in the guest** (`/usr/local/bin/agentvm-pty`, Python 3 already present): for each
  connection it reads a JSON header `{"cmd":"claude"|"shell","cols","rows"}` and starts the
  command in a PTY as user `agent` in `/home/agent/work`. Frames from the client:
  `[type:1 byte][length:4 bytes BE][payload]`, type 0 = input, 1 = resize `{"cols","rows"}`.
  From the guest to the client: raw PTY bytes.
- **Token**: stays in `/run/agentvm/token` (root, 0600). The PTY server (root) passes it to Claude
  only on fd 3 (`CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR`); shells do not receive it.
- **Agent state** via Claude Code hooks: `UserPromptSubmit` → `working`,
  `Stop`/`Notification` → `waiting`, written to `share/activity`. The supervisor reads it and
  exposes it as `activity` in the DTO.
- **Save / close** via request files in the shared folder:
  `save.request` → the guest does commit + `out.bundle` + `save.done`, the server does a forced
  `git fetch` onto `agent/<id>`; `close.request` → final save, `result.json`, shutdown.
- **No timeout** for terminals: the user closes them. "Stop" remains the forced stop.
- **VM limit** computed from RAM: `(RAM − 8 GB) / memory_mb`, overridable with
  `AGENTVM_CONCURRENCY`.
- **xterm.js** bundled in the binary (vendored), no external resources.
- The non-interactive mode (prompt → branch) remains available via the API, and the existing tests
  keep covering it.

## New APIs

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/tasks` | `interactive: true` opens a terminal; `prompt` optional |
| `GET` | `/api/tasks/{id}/pty?cmd=claude\|shell&cols&rows` | WebSocket to a PTY in the VM |
| `POST` | `/api/tasks/{id}/save` | saves the work to the branch without closing |
| `POST` | `/api/tasks/{id}/close` | saves and shuts down the VM |

## Tests (no mocks)

- Real VM: shell via vsock (`echo ciao` → `ciao`), `close.request` → clean shutdown.
- System: interactive terminal with an initial prompt → state `waiting` → `save` → the branch
  contains the file → `close` → `done`.
