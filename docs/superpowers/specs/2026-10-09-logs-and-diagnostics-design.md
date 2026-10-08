# agentvm — Logs, diagnostics, VM telemetry and Claude's history

Date: 2026-10-09 · Status: approved in chat

## Goal

Three parts, built and shipped in this order, each with its own tests:

1. **Logs and diagnostics**: when a VM fails, the dashboard says why and shows the evidence;
   the server keeps a structured log of what it did.
2. **VM telemetry**: accurate, never stale without saying so, complete (CPU, memory with what the
   balloon gave back, disk space and I/O, network, processes by CPU and by memory), with the
   whole life of the VM kept, not only the last minute.
3. **Claude's history**: the whole conversation (messages, tools, results) and its usage over
   time (tokens per turn, cost, context), kept after the VM is closed.

## Server log

- `tracing` + `tracing-subscriber` (JSON) + `tracing-appender` (daily files, 14 kept), set up in
  `src/logging.rs` and called once from `main.rs`. Files: `~/AgentVMs/logs/agentvm.log.<date>`,
  one JSON object per line: `timestamp`, `level`, `target`, `fields` (`message`, `task`, …).
  stderr keeps a human-readable line per event.
- The domain never logs (no I/O). The app layer logs with `tracing::{info,warn,error}!` and
  always puts the VM's id in a `task` field. Logged events:
  - state changes (`store.apply`): from → to, and the reason of a failure (warn);
  - VM process spawned (pid) and exited (exit, duration);
  - guest requests (save / sync / close): duration and outcome (warn on timeout);
  - import of the work (branch, commits) and its failure (error);
  - interrupted disk kept as a snapshot (warn); waiting for memory; disk too full (warn);
  - external command timeouts (warn); recovery at start-up (attached / finished / queued).
- `AGENTVM_LOG` sets the level (`info` by default; e.g. `debug`).

## Diagnostics of one VM

`GET /api/tasks/{id}/diagnostics` →

```json
{ "summary": "…the failure reason…", "hint": "…what to do, or null…",
  "timeline": [{"state": "queued", "at": 1791500033.0}, …],
  "logs": [{"name": "Job (inside the VM)", "file": "share/job.log", "tail": "…"}, …],
  "server_log": "…this VM's lines of the server log…" }
```

- **timeline**: the record keeps `timeline: Vec<{state, at}>` (state kind only: `queued`,
  `preparing`, `booting`, `running`, `collecting`, `done`, `no_changes`, `failed`, `stopped`),
  appended by `store.apply`, persisted with the record (`#[serde(default)]` for old records).
- **logs**: last 200 lines (at most 64 KB each) of `share/job.log`, `share/setup.log`,
  `share/claude.err`, `console.log`, `vm.events`; files that do not exist are left out. Read with
  `guestfs::read_suffix` (never a symlink, never a FIFO).
- **server_log**: the lines of today's and yesterday's server log files whose `task` field is
  this id, last 200, read from the last 8 MB of each file.
- **hint**: a pure domain function `domain::diagnosis::hint(reason, job_log) -> Option<String>`
  recognising the known failures: setup.sh failed, Claude Code install failed, timeout, VM error
  (boot), guest error, import (fetch) failed, disk full, missing token, interrupted (disk kept as
  a snapshot), the final save failed.

## Dashboard

- A **Diagnostics** panel in the machine view (`js/views/machine/diagnostics.js`): opened by a
  toolbar button for any VM, and shown at once under the outcome when a VM failed. Sections:
  why (summary + hint), timeline (state, local time, time since the previous step), one
  collapsible block per log (the failing one open), the server's lines.
- **Copy diagnostics**: puts the whole report as plain text on the clipboard.

## Testing

- adapters/logging: a JSON line written through the set-up writer carries `task`; the server
  log reader returns only that task's lines, from the end, bounded.
- domain: `hint` for each known failure; none for an unknown reason.
- app: the timeline of a task through its states, persisted and reloaded; diagnostics of a
  failed task built from real files (job.log, setup.log, a symlink planted as claude.err is
  ignored).
- http: `/api/tasks/{id}/diagnostics` shape; 404 for an unknown id.
- system (real VM): a repository whose `.agentvm/setup.sh` exits 1 and whose task then fails
  for another reason → the diagnostics name setup.sh in the hint and include setup.log.

## Part 2 — VM telemetry

- **Collector** (`guest/agentvm-metrics`, still once a second): adds `disk_read_bps` and
  `disk_write_bps` (from `/proc/diskstats`, the root disk), `top_mem` (3 processes using the most
  memory) next to `top` (by CPU), and `at` (the guest's wall clock) .
- **Freshness**: the host notes when it last read a new sample; the task carries
  `metrics_age_s`. Over 5 s the dashboard shows the numbers as stale ("no data for 12 s")
  instead of as live.
- **Memory**: the task carries `memory_limit_mb`, what the balloon currently lets the VM keep;
  the panel shows used / allowed / configured.
- **History**: in memory, the last 5 minutes at 1 s (live charts); on disk, one averaged line
  every 10 s for the VM's whole life in `<job>/telemetry.jsonl` (cpu, memory used and allowed,
  disk used, disk read/write, network in/out, load): about 1.5 MB a day. A domain type
  `TelemetrySample` and a pure `Downsampler` (10 s averages, peak kept for CPU) build the lines;
  an adapter appends them (atomic per line, never through a guest-planted symlink).
- **API**: `GET /api/tasks/{id}/telemetry?range=5m|1h|all` → series for each measure (`all`
  thinned to at most 1000 points).
- **Panel**: CPU, memory, disk I/O and network charts with a range switch (5 min / 1 h / whole
  life), stale state shown, processes by CPU and by memory.

## Part 3 — Claude's history

- **Kept from the guest**: a guest service (under `keep_running`) copies Claude Code's session
  files (`/root/.claude/projects/*/*.jsonl`) to `/mnt/job/claude/` when they change (every 2 s,
  and on save and close). They stay in the job folder after the VM ends (until "Delete logs").
- **Usage over time**: every change of `usage.json` (interactive) and every `result` event
  (automatic tasks) appends `{at, cost_usd, input_tokens, output_tokens, context_pct}` to
  `<job>/usage.jsonl`. Tokens per turn come from the transcript's assistant messages.
- **Domain**: `domain::transcript::parse_line` turns a session line into `HistoryEntry { at,
  kind: User | Assistant | ToolUse{name, input} | ToolResult{is_error, output}, usage }`, texts
  clipped to 16 KB; unknown lines are skipped.
- **API**: `GET /api/tasks/{id}/claude?after=<n>` → entries from index `n` (pages of 500), and
  `GET /api/tasks/{id}/claude/usage` → the series.
- **Dashboard**: a **Claude** tab in the machine view: the conversation (user and assistant
  messages, each tool call with its input and a folded result), token and cost charts over the
  turns, context used; available after the VM is closed.

## Testing (parts 2 and 3)

- domain: downsampling (averages, CPU peak, gaps); transcript lines of each kind, clipping,
  unknown lines.
- adapters: telemetry and usage files appended and read back; a symlink planted for them is
  never followed.
- app: freshness (a sample older than 5 s marks the task stale); series per range.
- http: `/telemetry`, `/claude`, `/claude/usage` shapes.
- system (real VM): disk I/O seen while the guest writes a file; an interactive VM's
  conversation is in the history after a prompt and still there after the VM is closed.

## Out of scope

Metrics export (Prometheus), log shipping, a log viewer for the whole server.
