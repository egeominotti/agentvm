# Native MCP control

`agentvm-mcp` is a native ARM64 Rust MCP server using the official SDK and stdio.
It connects to the existing AgentVM engine. Start AgentVM before connecting.
Disconnecting an MCP client leaves the engine and its VMs running.

The command-line build installs `bin/agentvm-mcp`. The Mac application includes
it under `Contents/Resources/runtime/bin/agentvm-mcp`; Settings exposes its client
configuration. Use an absolute binary path in your MCP client:

```json
{
  "mcpServers": {
    "agentvm": {
      "command": "/absolute/path/to/agentvm/bin/agentvm-mcp",
      "args": ["--url", "http://127.0.0.1:7777"]
    }
  }
}
```

The default URL is `http://127.0.0.1:7777`. `AGENTVM_URL` also selects an engine;
`--url` takes precedence. The binary only accepts loopback HTTP engine origins.

To control a Mac from another computer, transport stdio over authenticated SSH:

```json
{
  "mcpServers": {
    "agentvm-remote": {
      "command": "ssh",
      "args": ["-T", "your-mac", "/absolute/path/to/agentvm-mcp"]
    }
  }
}
```

SSH must provide non-interactive access and the host Keychain must be available
when operations require credentials. MCP stdout carries protocol messages only.
No public HTTP MCP listener is needed.

The server exposes 43 tools and the `agentvm://api` reference resource:

- Inspect the host, CLI catalog, tasks, diagnostics, telemetry and agent output.
- Launch, save, close, stop, delete and push VM work.
- Create, restore, export, import, back up and delete snapshots.
- Read/update settings, save/remove provider credentials, manage storage and the
  golden image; account import and browser login use `agentvm_api`.
- Run literal argv with `agentvm_exec`, or an explicit Bash script with
  `agentvm_shell`. Read/cancel an execution using its returned `exec_id`.
- List/stat/read/write/move/remove guest files, including binary base64 chunks.
- List/create/capture/input/resize persistent tmux terminal sessions.
- Stream snapshot archives and VM uploads through explicit host file paths with
  `agentvm_transfer`. Existing destination files are not overwritten on export.

All tools take explicit VM, snapshot or execution IDs. VM processes belong to
the engine. Process output is bounded and returned as base64 with truncation
flags; default process timeout is 60 seconds, maximum 30 minutes. File chunks
are at most 1 MiB. Use offsets for larger transfers.

Example `agentvm_launch` arguments:

```json
{
  "repo_path": "/Users/you/project",
  "interactive": true,
  "agent": {"kind": "codex", "auth": "account", "account_kind": "chatgpt"},
  "prompt": "Inspect the failing tests"
}
```

For provider access, use `"agent":{"kind":"codex","auth":"api","provider":"openai"}`.
Models are automatic; `model` is an optional API override. Account access requires
a connected account with the relevant entitlement, while API access uses the
selected provider's credential and billing plan.

Read `GET /api/agents` through `agentvm_api` for supported account/provider choices
and connection status. For an account offering `can_import`, call
`POST /api/settings/accounts/{kind}/import`. For `can_login`, call
`POST /api/settings/accounts/{kind}/login`, show its `verification_url` and optional
`user_code`, then poll `GET /api/settings/accounts/{kind}/login/{id}` until connected
or failed. The user completes browser authorization. `DELETE
/api/settings/accounts/{kind}` disconnects AgentVM without signing the host CLI out.

Example `agentvm_exec` arguments:

```json
{"id":"VM_ID","argv":["git","status","--short"],"cwd":"/root/work"}
```

`agentvm_output` accepts `{id,cursor,limit}`. Begin at cursor 0, then pass
`next_cursor`. Pages contain sequenced agent/state events, `more`, and `dropped`
for unavailable earlier output. Normalized output is redacted and persisted
on the host across engine restarts. Interactive CLI screens are available via
terminal capture; automated conversations use the output stream.

Structured process/file/terminal control is shipped to new VMs at launch.
VMs already running with an older guest runtime report unsupported control;
create a new VM or restore its snapshot to receive the current runtime.
