# agentvm MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Local web dashboard that launches Claude Code tasks in disposable Debian 13 arm64 VMs and returns an `agent/<id>` branch in the local repo.

**Architecture:** Rust server (axum/tokio, lib + bin) with a pure core (`domain`), orchestration (`app`), I/O adapters (`adapters`) and `http`. One Swift `agentvm-vm` process per VM (Virtualization.framework). The guest runs `agentvm-job` via systemd and communicates only through the virtiofs folder `/mnt/job`.

**Tech Stack:** Rust 1.98 (axum 0.8, tokio 1, serde, serde_json, thiserror 2, anyhow, futures, tokio-stream, tempfile dev), Swift 6.4 (Virtualization.framework), Bash/systemd in the guest, git CLI, `security` CLI.

**Spec:** `docs/superpowers/specs/2026-10-08-agentvm-mvp-design.md`

## Global Constraints

- Server on `127.0.0.1`, default port `7777`.
- Data in `~/AgentVMs/{images,golden,jobs}`; override with `AGENTVM_HOME`.
- Defaults: concurrency 4, 4 vCPU, 4096 MB, timeout 1800 s.
- Token: Keychain `security find-generic-password -s agentvm -w`, env fallback `CLAUDE_CODE_OAUTH_TOKEN`. Never in logs, never `set -x`.
- Result branch: `agent/<id>`, `<id>` = `YYYYMMDD-HHMMSS-xxxx` (4 random hex).
- No mocks/stubs/fakes: tests against real git, files, Keychain, VMs and Claude. Traits only with ≥2 real implementations.
- Dependencies: `http → app → domain`; `adapters` do not know about each other; `domain` without I/O.
- No installs on the host beyond the project binaries and Cargo crates.

## Review Focus

1. **Repo with uncommitted changes** → the agent starts from `HEAD`, the local working tree is not touched (test in Task 2: `fetch_bundle` does not modify working tree files).
2. **Prompt with quotes, newlines, `$()`** → reaches Claude intact, no shell execution (task.json via serde, `jq -r` in the guest; test in Task 1 on the `TaskSpec` round-trip with prompt `"it's $(rm -rf /)\nok"`).
3. **Two SSE clients on the same task, one connecting midway** → both receive the full history without duplicates (test in Task 6 on `EventLog::subscribe`).
4. **Stop of a task still in the queue** → moves to `stopped` without starting a VM (test in Task 1 on `transition(Queued, StopRequested)`).
5. **Truncated or non-JSON `stream.jsonl` line** → ignored as `AgentEvent::Unparsed`, following continues (test in Task 1).

---

### Task 1: Scaffold + pure domain

**Files:**
- Create: `server/Cargo.toml`, `server/src/lib.rs`, `server/src/domain/{mod,ids,task,agent_event,outcome,spec}.rs`, `server/tests/fixtures/stream-hello.jsonl` (copied from `~/AgentVMs/task1/job/stream.jsonl`, 0 tokens)
- Create: `.gitignore` (`/server/target`, `/bin`)

**Interfaces — Produces:**
- `ids`: `TaskId(String)` with `TaskId::generate(now: SystemTime, rand: [u8;2])` and `branch() -> String` (`agent/<id>`); `CommitSha::parse(&str) -> Result<CommitSha, IdError>` (40 hex); `RepoPath::new(PathBuf) -> Result<RepoPath, IdError>` (only checks that `<path>/.git` exists, no process); `Prompt::new(String) -> Result<Prompt, IdError>` (non-empty after trim).
- `task`: `enum TaskState { Queued, Preparing, Booting, Running, Collecting, Done{branch:String, commits:u32}, NoChanges, Failed{reason:String}, Stopped }` (serde tag `"state"`, snake_case); `enum TaskEvent { SlotAcquired, Prepared, VmStarted, VmExited, Finished(Final), Failure(String), StopRequested }`; `fn transition(&TaskState, &TaskEvent) -> Result<TaskState, InvalidTransition>`; `TaskState::is_terminal()`.
- `outcome`: `struct GuestResult { status: GuestStatus /*ok|no_changes|failed*/, claude_exit: i32, commits: u32, error: Option<String> }` (Deserialize); `enum VmExit { Clean, Error(String), Signaled }`; `struct OutcomeInput { exit: VmExit, result: Option<GuestResult>, stop_requested: bool, timed_out: bool, has_out_bundle: bool }`; `enum Final { Done{commits:u32}, NoChanges, Failed(String), Stopped }`; `struct Outcome { final_: Final, fetch: bool }`; `fn decide(&OutcomeInput) -> Outcome`.
- `agent_event`: `enum AgentEvent { Init{model:String}, Text{text:String}, ToolUse{name:String, summary:String}, ToolResult{is_error:bool, summary:String}, Retry{attempt:u32}, Result{is_error:bool, duration_ms:u64, cost_usd:f64, text:String}, Unparsed{raw:String} }` (Serialize, tag `"kind"`); `fn parse_line(&str) -> Vec<AgentEvent>` (an assistant message can contain multiple blocks). Summaries truncated to 300 characters.
- `spec`: `struct TaskSpec { id, prompt, branch, base_sha, timeout_s }` (Serialize/Deserialize) = `task.json`.

`decide` rules (in order): stop → `Stopped`/no fetch; timeout → `Failed("timeout")`; `VmExit::Error(m)` → `Failed("vm_error: m")`; result `None` → `Failed("guest_no_result")`; `ok`+bundle → `Done`, fetch; `ok` without bundle → `Failed("missing_out_bundle")`; `no_changes` → `NoChanges`; `failed` → `Failed(error.unwrap_or("claude_exit N"))`, fetch = `has_out_bundle`.

Valid transitions: Queued→Preparing (SlotAcquired), Preparing→Booting (Prepared), Booting→Running (VmStarted), Running→Collecting (VmExited), Booting→Collecting (VmExited), Collecting→terminal (Finished), any non-terminal→Failed (Failure), Queued→Stopped (StopRequested); StopRequested on the other non-terminal states leaves the state unchanged (the actual stop arrives with Finished(Stopped)). Everything else → `InvalidTransition`.

- [ ] Step 1: unit tests in each module: `transition` (all the cases above, incl. `Queued+StopRequested → Stopped`, `Done+VmStarted → Err`); `decide` (one test per rule); `parse_line` on the 6 lines of the real fixture → contains `Init`, at least one `ToolUse{name:"Write"|"Bash"}`, `Result{is_error:false}`; `parse_line("{\"type\":\"assi")` → `[Unparsed]`; serde round-trip of `TaskSpec` with the prompt `"it's $(rm -rf /)\nok"`; `TaskId::generate` format and `branch()`.
- [ ] Step 2: `cargo test` → FAIL (empty modules).
- [ ] Step 3: implement.
- [ ] Step 4: `cargo test` → PASS.
- [ ] Step 5: commit `feat(domain): pure core (states, outcome, agent events)`.

### Task 2: Host adapters (git, jobdir, tail, keychain, secret)

**Files:**
- Create: `server/src/adapters/{mod,git,jobdir,tail,keychain}.rs`, `server/src/secret.rs`, `server/tests/adapters.rs`

**Interfaces — Produces:**
- `Secret` (`secret.rs`): `Secret::new(String)`, `expose(&self) -> &str`, `Debug` → `[REDACTED]`, no `Display`.
- `Git { repo: RepoPath }`: `rev_parse(&self, rev:&str) -> Result<CommitSha, GitError>`; `bundle_all(&self, dest:&Path)`; `fetch_bundle(&self, bundle:&Path, branch:&str)` (`git fetch <bundle> <branch>:<branch>`); `diff(&self, base:&CommitSha, branch:&str) -> Result<String, GitError>`; `commit_count(&self, base, branch) -> u32`. All via `git -C <repo>`.
- `JobWorkspace` (`jobdir.rs`): `create(jobs_root:&Path, id:&TaskId) -> io::Result<Self>` creates `<root>/<id>/share` (0777); `dir()`, `share()`, `disk()`, `console()`, `efivars()`, `config_path()`, `pid_path()`; `write_spec(&TaskSpec)`, `write_token(&Secret)` (`share/.token`, 0600), `clone_disk(golden:&Path)` (`clonefile(2)` via `extern "C"`), `read_result() -> Option<GuestResult>` (invalid JSON → `None`), `has_out_bundle()`, `out_bundle()`; `Drop` deletes `disk.raw`, `efivars`, `share/.token`, `share/repo.bundle`. `fn cleanup_orphans(jobs_root:&Path)` reads `*/vm.pid`, kills the PID only if `ps -p <pid> -o comm=` ends with `agentvm-vm`, deletes `disk.raw`.
- `tail_lines(path:PathBuf, stop: watch::Receiver<bool>) -> impl Stream<Item=String>` (`tail.rs`): 200 ms poll, waits for the file to exist, emits complete lines, at the end (stop=true) drains the rest and emits the last line even without `\n`.
- `Keychain { keychain: Option<PathBuf> }`: `read_token(&self) -> Result<Secret, KeychainError>` (`security find-generic-password -s agentvm -w [keychain]`; if that fails, env `CLAUDE_CODE_OAUTH_TOKEN`; otherwise `KeychainError::Missing` with a message containing `security add-generic-password -s agentvm -a agentvm -w`).

- [ ] Step 1: tests in `tests/adapters.rs`, all with tempdirs and real tools:
  - `git_roundtrip`: temp repo with 1 commit + uncommitted file `dirty.txt`; `bundle_all`; `git clone` of the bundle into a second dir, `checkout -b agent/x`, commit `new.txt`, `git bundle create out.bundle <base>..agent/x`; `fetch_bundle` → `rev_parse("agent/x")` ok, `commit_count == 1`, `diff` contains `new.txt`, `dirty.txt` still present and uncommitted.
  - `workspace_clone_and_drop`: 1 MiB “golden” file → `clone_disk` → same content; drop → `disk.raw` gone, `share/stream.jsonl` written before the drop still present.
  - `workspace_token_perms`: `write_token` → mode `0o600`; `format!("{:?}", secret)` == `"[REDACTED]"`.
  - `read_result_invalid_json` → `None`.
  - `tail_follows_growing_file`: a process `sh -c 'for i in 1 2 3; do echo l$i; sleep 0.3; done; printf tail'` writes the file; the stream yields `["l1","l2","l3","tail"]`.
  - `keychain_temp`: `security create-keychain -p x <tmp>/t.keychain-db`, `add-generic-password -s agentvm -a agentvm -w tok123 <kc>` → `read_token` == `tok123`; empty keychain and env unset → `Missing`.
- [ ] Step 2: `cargo test --test adapters` → FAIL.
- [ ] Step 3: implement.
- [ ] Step 4: `cargo test` → PASS.
- [ ] Step 5: commit `feat(adapters): git, workspace, tail, keychain`.

### Task 3: Swift helper `agentvm-vm` + build

**Files:**
- Create: `vm-helper/{main,Config,MachineFactory,Runner}.swift`, `vm-helper/vz.entitlements`, `scripts/build.sh`

**Interfaces — Produces:** `bin/agentvm-vm --config <job.json>`; protocol §3.5 of the spec (keys `disk, efivars, share, console, cpus, memory_mb, seed_iso`); JSON Lines events on stdout `started` / `stopped{seconds}` / `error{message}`; exit 0 / 1 / 130; SIGTERM and SIGINT → `vm.stop` → 130. virtiofs tag `job`. Virtio console to a file. NAT. EFI store created if absent.
`scripts/build.sh`: `swiftc -O vm-helper/*.swift -o bin/agentvm-vm`, `codesign -s - -f --entitlements vm-helper/vz.entitlements bin/agentvm-vm`, `cargo build --release --manifest-path server/Cargo.toml`, copies `agentvm-server` into `bin/`.

- [ ] Step 1: `scripts/build.sh` → signed `bin/agentvm-vm` (`codesign -d --entitlements -` shows `com.apple.security.virtualization`).
- [ ] Step 2: `bin/agentvm-vm --config /nonexistent` → line `{"event":"error",...}` and exit 1.
- [ ] Step 3: commit `feat(vm-helper): Swift VM process with JSON protocol`.

### Task 4: Guest + golden image

**Files:**
- Create: `guest/agentvm-job`, `guest/agentvm.service`, `guest/10-agentvm.network`, `guest/setup-golden.sh`, `guest/golden-user-data.yaml`, `scripts/build-golden.sh`

**Interfaces:** `agentvm-job` implements §3.6/§4 of the spec: `set -euo pipefail`, no `set -x`; `trap` on EXIT that writes `result.json {status:"failed", error:"guest_error"}` if not already written, then `sync; poweroff -f`; without `task.json` → `error:"no_task"`; token read from `.token`, deleted, passed with `sudo --preserve-env=CLAUDE_CODE_OAUTH_TOKEN`; git run as `agent` with `user.name=agentvm`; DNS wait for `api.anthropic.com` max 10 s; auto-commit `agentvm: <id> (auto-commit)`; `out.bundle` only if `commits>0`; status `failed` if `claude_exit≠0`, `no_changes` if 0 commits, otherwise `ok`.
`build-golden.sh`: downloads and verifies `debian-13-genericcloud-arm64.tar.xz` (SHA512 from `SHA512SUMS`) only if absent, clones the disk to `golden/disk.raw.tmp`, grows it to 20 GiB, creates the seed ISO (`hdiutil makehybrid -iso -joliet -default-volume-name cidata`), copies `guest/*` into the share, starts `agentvm-vm` with `seed_iso`, checks for `GOLDEN_OK` in `setup.log`, renames to `golden/disk.raw`.

- [ ] Step 1: `scripts/build-golden.sh` → prints `GOLDEN_OK`, creates `~/AgentVMs/golden/disk.raw`.
- [ ] Step 2: commit `feat(guest): job runner and golden image build`.

### Task 5: `VmProcess` adapter + real VM test

**Files:**
- Create: `server/src/adapters/vm.rs`, `server/tests/vm.rs`

**Interfaces — Produces:** `VmConfig { disk, efivars, share, console, cpus:u32, memory_mb:u64, seed_iso:Option<PathBuf> }` (Serialize, §3.5 keys); `VmProcess::spawn(helper:&Path, config_path:&Path, cfg:&VmConfig) -> Result<VmProcess, VmError>` (writes the config, `kill_on_drop(true)`); `pid() -> u32`; `next_event(&mut self) -> Option<VmEvent>` with `enum VmEvent { Started, Stopped{seconds:f64}, Error(String) }`; `terminate(&self)` (SIGTERM via `extern "C" fn kill`); `wait(self) -> VmExit` (exit 0 → `Clean`, 130 → `Signaled`, other → `Error(last message or "exit N")`).

- [ ] Step 1: `#[ignore]` tests (require golden + `bin/agentvm-vm`): `boots_golden_without_task` → `Started`, then `wait()==Clean`, `result.json` with `error=="no_task"`, duration < 20 s; `terminate_running_vm` → after `Started` `terminate()` → `Signaled`.
- [ ] Step 2: `cargo test --test vm -- --ignored` → FAIL, then implement → PASS.
- [ ] Step 3: commit `feat(adapters): VmProcess`.

### Task 6: App (store, events, scheduler, supervisor)

**Files:**
- Create: `server/src/app/{mod,store,events,scheduler,supervisor}.rs`, `server/src/config.rs`

**Interfaces — Produces:**
- `Config { home, port:u16, concurrency:usize, cpus:u32, memory_mb:u64, timeout_s:u64, vm_helper:PathBuf }` with `Config::from_env()` (variables `AGENTVM_HOME|PORT|CONCURRENCY|CPUS|MEMORY_MB|TIMEOUT_S|VM_HELPER`; default helper = `agentvm-vm` next to the executable); `golden()`, `jobs()`.
- `events`: `enum StreamItem { State(TaskState), Agent(AgentEvent) }` (Serialize); `EventLog` with `push(StreamItem)` and atomic `subscribe() -> (Vec<(u64,StreamItem)>, broadcast::Receiver<(u64,StreamItem)>)`; the consumer discards seq ≤ the last one in the history.
- `store`: `TaskRecord { id, repo, prompt, base_sha, state, created_at, finished_at }`; `Store` (internal Arc<Mutex<HashMap>>) with `insert`, `apply(&TaskId, TaskEvent) -> Result<TaskState, ..>` (uses `transition` and does `push(State)`), `list()`, `get()`, `log(&TaskId) -> Arc<EventLog>`, `request_stop(&TaskId)`, `stop_signal(&TaskId) -> watch::Receiver<bool>`.
- `scheduler`: `Scheduler::new(concurrency)`; `acquire().await -> OwnedSemaphorePermit`.
- `supervisor`: `async fn run(ctx: Arc<AppCtx>, id: TaskId)` with `AppCtx { config, store, scheduler, keychain }`; `fn submit(ctx, repo, prompt, base_ref) -> Result<TaskId, SubmitError>` (validates `RepoPath`/`Prompt`, `rev_parse`, checks token and golden *before* queuing, spawns `run`). Flow: permit → (stop? → `Finished(Stopped)`) → `SlotAcquired` → workspace, bundle, spec, token, clone → `Prepared` → spawn VM, `vm.pid` → `Started` → `VmStarted` + tail of `stream.jsonl` → `select!{ vm exit, stop, timeout(timeout_s) }` → `VmExited` → `decide` → fetch if requested (error → `Failed("fetch_failed: …")`) → `Finished`. Preparation errors → `Failure(msg)`.

- [ ] Step 1: real unit tests in `events.rs` (`subscribe` midway: history + live without duplicates, two subscribers) and in `store.rs` (out-of-order apply → error, stop while queued → `Stopped`). The supervisor is covered by the system test (Task 7).
- [ ] Step 2: `cargo test` → FAIL, implement → PASS.
- [ ] Step 3: commit `feat(app): store, events, scheduler and supervisor`.

### Task 7: HTTP, dashboard, main + system test and launch

**Files:**
- Create: `server/src/http/{mod,routes,dto}.rs`, `server/src/http/web/index.html`, `server/src/main.rs`, `server/tests/system.rs`, `docs/e2e.md`
- Modify: `README.md` ("Usage" section)

**Interfaces:** routes from spec §5 (axum syntax `/{id}`), plus `GET /api/status` → `{golden: bool, token: bool, concurrency, running}`. JSON errors `{error: "..."}` with 400/404/409. SSE: `state` and `agent` events (JSON of `StreamItem`), full history then live, 15 s keep-alive. `main.rs`: `Config::from_env`, `cleanup_orphans`, bind `127.0.0.1:<port>`, logs to stderr without secrets. Dashboard: one HTML file with vanilla JS, form (repo, prompt, ref), list refreshed every 2 s, details with SSE log and diff; warnings if `golden=false` or `token=false` with the command to run; color-coded state; light/dark theme.

- [ ] Step 1: `server/tests/system.rs` `#[ignore]` (real server, VM and Claude): starts `bin/agentvm-server` on an ephemeral port with the real `AGENTVM_HOME`; temp repo with one commit; `POST /api/tasks` prompt "Create hello.txt containing ciao and commit it"; poll `/api/tasks/{id}` until terminal (max 5 min) → `done`; `git -C repo show agent/<id>:hello.txt` contains `ciao`; `/diff` contains `hello.txt`; second task + `POST /stop` right after `running` → `stopped`; grep for `sk-ant-` in `~/AgentVMs/jobs` → 0. All via `curl` and the `git` CLI.
- [ ] Step 2: `scripts/build.sh && cargo test -- --ignored` → PASS.
- [ ] Step 3: 4 tasks in parallel from the dashboard → 4 `done`.
- [ ] Step 4: commit `feat(http): API, dashboard and system test`.
- [ ] Step 5: start `bin/agentvm-server` and open `http://127.0.0.1:7777`.
