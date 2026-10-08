# agentvm — MVP Design

Date: 2026-10-08 · Status: draft pending approval

## 1. Goal

Launch multiple **Claude Code** tasks from a local web dashboard, each running fully
autonomously inside a **real Debian 13 arm64 VM** on Apple Silicon. The result of each task
comes back to the local repo as an `agent/<id>` branch, ready for review.

**User**: a single developer, on their own Mac (M5 Max, 18 cores, 64 GB).

**MVP success criteria**
1. From the dashboard I enter a local repo + prompt and press "Start": a VM boots and the agent gets to work.
2. I see the agent's logs live (messages, commands run) in the browser.
3. When the task finishes I find `agent/<id>` in my repo, with the diff visible in the dashboard.
4. I can launch at least 4 tasks in parallel without interference.
5. I can stop a running task.
6. The Claude token never appears in logs, job files or output.
7. Nothing is installed on the macOS host (only the project binaries and the files in `~/AgentVMs`).

## 2. What the prototype proved (2026-10-08)

| Check | Result |
|---|---|
| Debian 13.7 arm64 (`genericcloud`, `.raw`) booted with Virtualization.framework, EFI | ✅ |
| Ad-hoc signed binary with the `com.apple.security.virtualization` entitlement (no Apple account) | ✅ |
| virtiofs in the `6.12-cloud-arm64` kernel, read/write from the Mac | ✅ |
| Claude Code 2.1.294 (native installer) + `claude -p` with `CLAUDE_CODE_OAUTH_TOKEN` | ✅ |
| Golden image + APFS clone: clone 0.01 s, VM ready with the job 2.9 s, full cycle 12 s (8 s of which Claude) | ✅ |

**Lessons built into the design**
- cloud-init writes a network config tied to the golden's MAC → the clones end up without network.
  The golden uses a generic systemd-networkd config (`Name=en*`, DHCP) and no cloud-init at runtime.
- `/etc/machine-id` must be emptied in the golden, otherwise the clones request the same DHCP lease.
- `set -x` printed the token into a log: guest scripts never use tracing and read the token
  from a file, deleting it immediately.
- Time was dominated by apt and installers: everything goes into the golden, never at runtime.

## 3. Architecture

```
Browser ──HTTP + SSE──> agentvm-server (Rust: axum + tokio)          127.0.0.1:7777
                          ├── REST API + static dashboard (embedded HTML/JS, no frontend build)
                          ├── task queue + concurrency limit (default 4)
                          ├── git: inbound bundle, outbound bundle fetch, diff
                          ├── tail of stream.jsonl → SSE events to the browser
                          └── starts 1 process per VM ─┐
                                                       ▼
                          agentvm-vm (Swift, ~200 lines, derived from the prototype)
                             input : --config job.json
                             output: JSON events on stdout, one line per event
                             stop  : SIGTERM → forced VM stop
                                                       │ Virtualization.framework
                                                       ▼
                          Debian 13 VM (APFS clone of the golden)
                             agentvm.service → /usr/local/bin/agentvm-job
                             /mnt/job (virtiofs) = ~/AgentVMs/jobs/<id>/share
```

**Why two languages**: Virtualization.framework is a Swift/Objective-C API. The VM code stays
native and already verified; Rust handles the server, concurrency and git. Each VM is a separate process,
so a VM crash stops neither the server nor the other VMs. Only `agentvm-vm` needs the entitlement.

### 3.1 Components

| Unit | Language | Responsibility | Depends on |
|---|---|---|---|
| `vm-helper/` → `agentvm-vm` | Swift | Starts and stops a VM given a JSON config; emits events | Virtualization.framework |
| `server/` → `agentvm-server` | Rust | API, dashboard, queue, task lifecycle, git | `agentvm-vm`, `git` CLI, `security` CLI |
| `guest/` | Bash + systemd | Scripts and units installed in the golden: runs the job in the VM | Claude Code, git |
| `scripts/build.sh` | Bash | Compiles Rust + Swift, signs `agentvm-vm` | cargo, swiftc, codesign |
| `scripts/build-golden.sh` | Bash | Downloads Debian, verifies SHA512, creates the golden with cloud-init (once) | `agentvm-vm`, `hdiutil`, curl |

### 3.2 Architectural principles

1. **Functional core, imperative shell.** Decisions (state transitions, interpretation of
   `result.json`, parsing of Claude events, construction of git commands) are pure functions over
   data. I/O (processes, files, git, network) lives in a thin shell that executes the decisions made
   by the core. The core is tested with real data, without substitutes.
2. **One-way dependencies**: `http → app → domain`. The adapters (`vm`, `git`,
   `keychain`, `jobdir`) are used by `app` and know neither HTTP nor the other adapters.
   `domain` depends on nothing in the project and does no I/O.
3. **No abstraction just for testing.** A trait exists only if there really are two
   production implementations. Adapters are concrete `struct`s. Tests use the real
   implementations (see §10).
4. **Invalid states unrepresentable.** Newtypes for `TaskId`, `CommitSha`, `RepoPath` (validated
   at construction: it is a git repo) and `Prompt` (non-empty). The task state is an `enum` carrying
   each state's own data (e.g. `Running { started_at }`, `Failed { reason }`). Transitions go through
   a single function `transition(state, event) -> Result<State, InvalidTransition>`.
5. **Typed secrets.** `Secret<String>` does not implement `Display`; its `Debug` prints `[REDACTED]`.
   The value is read only via `expose()`, used in a single place: writing `.token`.
6. **One responsibility per module, small files.** If a file exceeds ~300 lines or mixes different
   levels, it gets split.
7. **Typed errors at the boundaries.** Each module has its own `enum Error` (`thiserror`). `anyhow` only
   in `main`. Every error that reaches the user has an actionable message.
8. **Cleanup guaranteed by RAII.** `JobWorkspace` owns the job folder and the cloned disk:
   `Drop` deletes the disk even on panic or error. `VmProcess` terminates the child process in
   `Drop`.
9. **No global state.** The configuration (`Config`: paths, port, concurrency, CPU/RAM,
   timeout) is read once in `main` and passed via constructors.

### 3.3 Patterns adopted (and where)

| Pattern | Where | Why |
|---|---|---|
| **State machine** | `domain::task` | Explicit lifecycle, transitions checked in a single place |
| **Supervisor (actor)** | `app::supervisor` | One tokio task per task owns its whole lifecycle; no shared lock on the flow |
| **Publish/subscribe** | `app::events` (`tokio::sync::broadcast`) | Multiple SSE clients follow the same task without coupling to the supervisor |
| **Bounded scheduler** | `app::scheduler` (`Semaphore`) | Concurrency limit in a single place |
| **Repository** | `app::store` | Sole owner of task state (in memory for the MVP; replaceable with SQLite without touching the rest) |
| **Adapter** | `vm`, `git`, `keychain`, `jobdir` | Isolate an external system behind a small, typed API |
| **RAII guard** | `JobWorkspace`, `VmProcess` | Guaranteed resource cleanup |
| **Newtype** | `domain::ids` | Validation at the boundary, expressive types |

### 3.4 Rust server modules

```
server/src/
├── main.rs              reads Config, builds the components, starts axum (wiring only)
├── config.rs            Config + loading from env/flags
├── domain/              pure, no I/O
│   ├── ids.rs           TaskId, CommitSha, RepoPath, Prompt
│   ├── task.rs          Task, TaskState, Event, transition()
│   ├── agent_event.rs   parsing of a stream.jsonl line → AgentEvent (text, tool_use, result…)
│   └── outcome.rs       interpretation of result.json + process exit → final state
├── app/                 use cases, orchestration
│   ├── store.rs         task repository
│   ├── scheduler.rs     queue + semaphore
│   ├── supervisor.rs    lifecycle of a task (prepare → boot → follow → collect → cleanup)
│   └── events.rs        per-task broadcast
├── adapters/            I/O to external systems
│   ├── vm.rs            VmProcess: spawn of agentvm-vm, events from stdout, SIGTERM
│   ├── git.rs           rev-parse, bundle create, fetch from bundle, diff (git CLI)
│   ├── jobdir.rs        JobWorkspace: job folder, clonefile, §3.6 contract files
│   ├── keychain.rs      token read → Secret
│   └── tail.rs          tail of a growing file → Stream of lines
└── http/                axum: routes, DTOs, SSE, static assets
    ├── routes.rs
    ├── dto.rs
    └── web/index.html
```

The Swift code (`vm-helper/`) follows the same split: `Config.swift` (Codable + validation),
`MachineFactory.swift` (VZ configuration), `Runner.swift` (lifecycle and events), `main.swift`
(wiring only). Guest scripts use `set -euo pipefail` and never use `set -x`.

### 3.5 `agentvm-vm` protocol

Config (`job.json`, written by the server):
```json
{ "disk": ".../disk.raw", "efivars": ".../efivars", "share": ".../share",
  "console": ".../console.log", "cpus": 4, "memory_mb": 4096, "seed_iso": null }
```
Events on stdout (JSON Lines): `{"event":"started"}`, `{"event":"stopped","seconds":12.1}`,
`{"event":"error","message":"..."}`. Exit code: 0 if the VM shut down on its own, 1 if there was an error, 130 if it was stopped.
`seed_iso` is only used by `build-golden.sh`.

### 3.6 Shared folder contract (`/mnt/job`)

| File | Written by | Content |
|---|---|---|
| `task.json` | server | `{id, prompt, branch, base_sha, timeout_s}` |
| `repo.bundle` | server | `git bundle create --all` of the local repo |
| `.token` | server | OAuth token; the agent reads it and deletes it immediately |
| `stream.jsonl` | guest | output of `claude -p --output-format stream-json --verbose` |
| `out.bundle` | guest | `git bundle create out.bundle <base_sha>..<branch>` (absent if there are no commits) |
| `result.json` | guest | `{status: "ok"\|"no_changes"\|"failed", claude_exit, commits, error?}` |
| `job.log` | guest | guest script log (no tracing, no token) |

## 4. Task lifecycle

```
queued → preparing → booting → running → collecting → done | no_changes | failed | stopped
```

1. **queued**: `POST /api/tasks {repo_path, prompt, base_ref?}`. The server validates that `repo_path`
   is a git repo and resolves `base_ref` (default `HEAD`) to `base_sha`. Uncommitted changes
   in the working tree are not included: the agent always starts from a commit.
2. **preparing** (when a slot is free): creates `~/AgentVMs/jobs/<id>/`, writes `repo.bundle` and
   `task.json`, reads the token from the Keychain (`security find-generic-password -s agentvm -w`) or from the
   `CLAUDE_CODE_OAUTH_TOKEN` variable and writes `.token`, then `clonefile(golden/disk.raw → disk.raw)`.
3. **booting**: starts `agentvm-vm`, `started` → **running**.
4. **running**: inside the VM `agentvm-job` clones `repo.bundle` into `/home/agent/work`, checks out
   `base_sha` on a new branch `agent/<id>`, waits for DNS (max 10 s) and runs
   `claude -p "$prompt" --dangerously-skip-permissions --output-format stream-json --verbose`
   as user `agent`. The server follows `stream.jsonl` and forwards each line over SSE.
5. **collecting**: the agent runs `git add -A && git commit` if uncommitted changes remain, creates
   `out.bundle` and `result.json`, then `sync; poweroff -f`. When the server receives `stopped`, it runs
   `git fetch <out.bundle> agent/<id>:agent/<id>` in the local repo.
6. **End**: `done` (branch created), `no_changes`, `failed` or `stopped`. The VM disk is
   deleted; `stream.jsonl`, `result.json` and `job.log` are kept for reference.

### 4.1 Error handling

| Case | Behavior |
|---|---|
| Task timeout (default 30 min) | SIGTERM to `agentvm-vm` → `failed` with reason `timeout` |
| User stop | SIGTERM → `stopped`; no fetch |
| `agentvm-vm` exits with an error or crashes | `failed`, last lines of `console.log` in the error |
| No `result.json` after shutdown | `failed` with reason `guest_no_result` |
| `claude` exits ≠ 0 | `failed` with Claude's `result` event, but the fetch still happens if there are commits |
| Branch `agent/<id>` already exists | impossible: `<id>` is unique (timestamp + random suffix) |
| Token missing | the task does not start: clear error in the dashboard with Keychain instructions |
| Server restart | state is in memory: running tasks are lost. Each job writes `vm.pid`; on startup the server kills PIDs still alive and deletes the disks left in `~/AgentVMs/jobs/` |

## 5. API

| Method | Path | Description |
|---|---|---|
| `GET` | `/` | Dashboard |
| `POST` | `/api/tasks` | Creates a task `{repo_path, prompt, base_ref?}` → `{id}` |
| `GET` | `/api/tasks` | Task list with state and duration |
| `GET` | `/api/tasks/:id` | Details, `result.json`, branch name |
| `GET` | `/api/tasks/:id/events` | SSE: `stream.jsonl` lines + state changes (replay from the start) |
| `GET` | `/api/tasks/:id/diff` | `git diff base_sha..agent/<id>` |
| `POST` | `/api/tasks/:id/stop` | Stops the task |

## 6. Dashboard (MVP)

A single HTML page with vanilla JS, embedded in the binary:
- "New task" form: repo path, prompt, optional base ref;
- task list: state, repo, start of the prompt, duration, Stop button;
- details: live logs rendered readable (assistant text, tool names and inputs, final result),
  diff of the produced branch.

## 7. Golden image

`scripts/build-golden.sh` (one-off, ~2 min):
1. downloads `debian-13-genericcloud-arm64.tar.xz` and verifies the SHA512 from `SHA512SUMS`;
2. clones the disk, grows it to 20 GB and boots it with a cloud-init seed (`hdiutil makehybrid`);
3. the seed installs `git curl ca-certificates ripgrep jq build-essential`, Claude Code as user
   `agent`, `agentvm-job` and `agentvm.service`, the generic network config; disables cloud-init,
   ssh, apt timers and the GRUB wait; cleans the apt cache and empties `machine-id`;
4. result: `~/AgentVMs/golden/disk.raw`.

## 8. Security

- The server listens only on `127.0.0.1`.
- Each VM sees only its own `share/` folder, never the real repo: code goes in and out via bundles.
- The token is read from the Keychain, written to `.token` with `600` permissions, read and deleted by the guest
  before starting Claude, never passed on a command line visible in logs and never traced.
- The VM disk, which contains the agent's environment, is deleted at the end of the task.
- The VMs have internet access via NAT: it is needed for the Claude API and to install dependencies.

## 9. Out of scope for the MVP

Pool of pre-booted VMs, direct kernel boot, merge button, SQLite persistence,
authentication and remote access, multiple golden images, CPU/RAM charts, submodules and Git LFS,
separate CLI, tasks that continue an existing session.

## 10. Tests — no mocks

**Rule**: no mocks, stubs, fakes or test doubles. Every test uses the real components: real git, real files,
real processes, real VMs, a real HTTP server, and real Claude where needed. Whatever is hard to test
without substitutes is made pure (§3.2, point 1) instead of being simulated.

| Level | What | With what | When it runs |
|---|---|---|---|
| **1. Pure core** | `transition()`, `outcome`, `agent_event` parsing, newtypes | Real data: `stream.jsonl`, `result.json` recorded from real runs (`server/tests/fixtures/`, checked to contain no token) | always (`cargo test`) |
| **2. Adapters** | `git`: bundle, fetch, diff on real temporary repos · `jobdir`: real clonefile on APFS, cleanup in `Drop` · `tail`: growing file written by a real process · `keychain`: real temporary keychain (`security create-keychain` in a tempdir) | Real system, isolated tempdirs | always |
| **3. VM** | `agentvm-vm` + `VmProcess`: boot of the real golden without `task.json` → the guest writes `result.json {status:"failed", error:"no_task"}` and shuts down; SIGTERM on a running VM → exit 130 and disk deleted | Real Debian VM (~4 s per test) | `cargo test -- --ignored` (requires the golden) |
| **4. System** | Real server on an ephemeral port → `POST /api/tasks` on a temporary repo with the prompt "create hello.txt containing ciao and commit it" → SSE until `done` → `agent/<id>` contains `hello.txt`; 4 tasks in parallel; stop of a running task; grep for the token across all of `~/AgentVMs` → 0 occurrences | Real server + VM + Claude (~$0.03 per task) | `cargo test -- --ignored` with the token in the Keychain |

Level 3 and 4 tests are `#[ignore]` only because they require the golden and the token, not because
they use anything fake. Level 4 replaces the manual end-to-end.

## 11. Repo structure

```
agentvm/
├── README.md
├── docs/superpowers/specs/2026-10-08-agentvm-mvp-design.md
├── vm-helper/      main.swift, Config.swift, MachineFactory.swift, Runner.swift, vz.entitlements
├── server/         Cargo.toml, src/ (see §3.4), tests/{adapters,vm,system}.rs, tests/fixtures/
├── guest/          agentvm-job, agentvm.service, 10-agentvm.network, golden-user-data.yaml
└── scripts/        build.sh, build-golden.sh
```
Runtime data: `~/AgentVMs/{images,golden,jobs}/`.
