# Logs, diagnostics, VM telemetry and Claude's history — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The dashboard explains every failure with its evidence, shows complete and honest VM
telemetry over the VM's whole life, and keeps Claude's whole conversation and usage.

**Architecture:** `tracing` for the server log (`src/logging.rs`); pure domain rules
(`diagnosis`, `telemetry`, `transcript`); adapters that read guest files safely and append
host-owned history files; app use cases behind three new endpoints; three dashboard panels.

**Tech Stack:** Rust (axum, tokio, serde, `tracing 0.1.44`, `tracing-subscriber 0.3.23` with
`json` + `env-filter`, `tracing-appender 0.2.5`), Python guest collector, vanilla JS dashboard.

**Spec:** `docs/superpowers/specs/2026-10-09-logs-and-diagnostics-design.md`

## Global Constraints

- Every file ≤ 300 lines (architecture test). Clean layers: the domain does no I/O and never
  logs; adapters never use each other; http stays thin.
- No mocks: real files, real processes, real VMs for the `#[ignore]` system tests.
- Guest-written files are read only through `guestfs` (no symlinks, no FIFOs, bounded reads).
  Host history files live outside `share/` and are written by the host only.
- Texts kept per entry are clipped to 16 KB (`domain::agent_event::MAX_TEXT`).
- New JSON fields on existing types use `#[serde(default)]`: records and metrics written by the
  previous version must still load.
- Push to `main` after each task (no branches); `scripts/test.sh` green before each commit;
  `scripts/test.sh --ignored` green at the end of each part.

## Review Focus

- **A VM started before this change** (old collector, no `claude/` copier): every panel still
  renders; missing measures show as "—", never as 0. Test in Tasks 5 and 6.
- **The server restarted mid-life**: telemetry history and Claude history continue from the
  files, nothing duplicated. Test in Tasks 6 and 8.
- **A huge transcript** (hours of work, 100 MB): the history API pages it without reading it
  whole. Test in Task 8.
- **Clock jumps** (sleep/wake of the Mac): the downsampler never produces a negative or giant
  interval. Test in Task 5.
- **A failed task whose job folder was cleaned** ("Delete logs"): diagnostics still answer with
  the summary and timeline, logs empty. Test in Task 3.

---

## Part 1 — Logs and diagnostics

### Task 1: Server log
**Files:** create `server/src/logging.rs`, `server/src/adapters/server_log.rs`; modify
`server/Cargo.toml`, `server/src/lib.rs`, `server/src/main.rs`, `server/src/adapters/mod.rs`,
`server/tests/architecture.rs` (adapter list); test `server/tests/adapters/server_log.rs`.
**Interfaces:** `logging::init(logs_dir: &Path, level: &str) -> WorkerGuard` (JSON daily files,
14 kept, plus human-readable stderr); `adapters::server_log::lines_for(logs_dir, task: &str,
max_lines: usize) -> Vec<String>` (today's then yesterday's file, last 8 MB of each, lines whose
`fields.task` equals `task`, oldest first, at most `max_lines`).
- [ ] Test: write three JSON lines (two for task A, one for B) into a dated file → `lines_for(A)`
  returns A's two in order; a 20 MB file is read from its end only (returns within 1 s).
- [ ] Implement; `main.rs` keeps the guard alive, level from `AGENTVM_LOG` (default `info`).
- [ ] `scripts/test.sh`; commit `feat: structured server log`.

### Task 2: What the server logs
**Files:** modify `app/store.rs` (state changes), `app/launch.rs` (spawn, waiting for memory),
`app/supervise/mod.rs` (exit + duration), `app/guest_channel.rs` (request outcome + duration),
`app/collect.rs` (import, kept disk), `app/recover.rs`, `app/context.rs` (disk full),
`process.rs` (timeouts).
- [ ] Each event: `tracing::info!/warn!/error!(task = %id, ...)` with the fields in the spec.
- [ ] Test (app): a task driven through its states with `logging::init` on a temp dir → its
  log lines contain `queued→preparing`… and the failure reason.
- [ ] commit `feat: the server logs what happens to every VM`.

### Task 3: Diagnostics use case and endpoint
**Files:** create `domain/diagnosis.rs`, `app/diagnostics.rs`, `http/diagnostics.rs`; modify
`app/record.rs` (`timeline: Vec<StateAt>`), `app/store.rs` (append on change), `http/router.rs`;
tests `tests/domain/diagnosis.rs`, `tests/app/diagnostics.rs`, `tests/http.rs`.
**Interfaces:** `domain::diagnosis::hint(reason: &str, job_log: &str) -> Option<String>`;
`app::diagnostics::of(ctx, id) -> Result<Diagnostics, DiagnosticsError>` with
`Diagnostics { summary, hint, timeline, logs: Vec<LogTail{name,file,tail}>, server_log }`.
- [ ] Tests: `hint` for setup failed / install failed / timeout / vm_error / guest_error /
  fetch_failed / disk full / no token / interrupted / final save failed, `None` otherwise;
  timeline persisted across `Store::load`; diagnostics from real files with a planted
  `claude.err` symlink (ignored); job folder deleted → summary and timeline still there.
- [ ] `GET /api/tasks/{id}/diagnostics`; 404 for unknown ids. commit.

### Task 4: Diagnostics panel
**Files:** create `web/js/views/machine/diagnostics.js`, `web/css/diagnostics.css`; modify
`machine.js` (toolbar button, auto-open on failure), `index.html`; system test in
`tests/system/diagnostics.rs` (+ `mod` in `main.rs`).
- [ ] Panel: why + hint, timeline with local times and step durations, a folding block per log
  (the failing one open), server lines, "Copy diagnostics".
- [ ] System test: repo with `.agentvm/setup.sh` doing `exit 1` and a prompt that fails →
  `/diagnostics` hint names setup.sh and `logs` holds setup.log.
- [ ] Browser check (headless Chrome, no console errors); `--ignored` suite; commit, push.

## Part 2 — VM telemetry

### Task 5: Collector and downsampling
**Files:** modify `guest/agentvm-metrics`, `domain/metrics.rs` (`disk_read_bps`,
`disk_write_bps`, `top_mem`, `at`, all `#[serde(default)]`); create `domain/telemetry.rs`
(`TelemetrySample`, `Downsampler::push(sample) -> Option<TelemetrySample>` every 10 s: averages,
CPU peak, intervals clamped to 0–60 s); tests `tests/domain/telemetry.rs`.
- [ ] Tests: 10 one-second samples → one averaged line with the CPU peak; a 2-hour jump (Mac
  asleep) closes the bucket without a giant average; an old-collector JSON still parses.
- [ ] commit.

### Task 6: Freshness, memory limit, history on disk, API
**Files:** create `adapters/telemetry_file.rs` (append/read `<job>/telemetry.jsonl`, no
symlinks), `app/telemetry.rs` (series per range), `http/telemetry.rs`; modify
`app/record.rs` (`metrics_at`, live history 300 samples), `app/supervise/tick.rs`,
`http/dto.rs` (`metrics_age_s`, `memory_limit_mb`).
- [ ] Tests: freshness (age grows when no new sample); file append/read and planted symlink
  refused; `range=all` thinned to ≤ 1000 points; restart continues the same file.
- [ ] commit.

### Task 7: Telemetry panel
**Files:** modify `web/js/views/machine/telemetry.js` (split into `telemetry/` modules if it
grows), `web/js/ui/sparkline.js` → a small line-chart module, `web/css/machine.css`.
- [ ] Load the `dataviz` skill first. Charts for CPU, memory (used / allowed / configured), disk
  I/O, network; range switch 5 min / 1 h / whole life; stale banner ("no data for 12 s");
  processes by CPU and by memory; "—" for measures an old VM does not report.
- [ ] System test: guest writes 200 MB with `dd` → `disk_write_bps` > 0 in `/telemetry`.
  `--ignored` suite; commit, push.

## Part 3 — Claude's history

### Task 8: Keeping the conversation and the usage
**Files:** modify `guest/agentvm-job` (`keep_running` a copier of
`/root/.claude/projects/*/*.jsonl` → `/mnt/job/claude/`, every 2 s and on save/close); create
`domain/transcript.rs` (`parse_line`), `adapters/history_file.rs` (usage.jsonl append/read,
transcript pages read from an offset without loading the whole file); modify
`app/supervise/tick.rs` (usage sample on change), `app/supervise/follower.rs` (result → sample).
- [ ] Tests: each transcript line kind, clipping, unknown lines skipped; a 100 MB transcript
  paged in bounded memory; usage samples appended once per change, kept after a restart.
- [ ] commit.

### Task 9: History API and Claude tab
**Files:** create `app/history.rs`, `http/history.rs`, `web/js/views/machine/claude.js`,
`web/css/claude.css`; modify `machine.js` (tab), `router.rs`.
- [ ] `GET /api/tasks/{id}/claude?after=n` (pages of 500) and `/claude/usage`.
- [ ] Tab: conversation (messages, tool calls with input, folded results), tokens per turn and
  cost over time, context used; works for a closed VM.
- [ ] System test: interactive VM with a prompt → its conversation is in `/claude`; after close,
  still there. `--ignored` suite; README; commit, push.
