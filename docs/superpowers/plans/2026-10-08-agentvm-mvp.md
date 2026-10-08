# agentvm MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dashboard web locale che lancia task Claude Code in VM Debian 13 arm64 usa-e-getta e restituisce branch `agent/<id>` nel repo locale.

**Architecture:** Server Rust (axum/tokio, lib + bin) con core puro (`domain`), orchestrazione (`app`), adapter I/O (`adapters`) e `http`. Un processo Swift `agentvm-vm` per VM (Virtualization.framework). Il guest esegue `agentvm-job` via systemd e comunica solo tramite la cartella virtiofs `/mnt/job`.

**Tech Stack:** Rust 1.98 (axum 0.8, tokio 1, serde, serde_json, thiserror 2, anyhow, futures, tokio-stream, tempfile dev), Swift 6.4 (Virtualization.framework), Bash/systemd nel guest, git CLI, `security` CLI.

**Spec:** `docs/superpowers/specs/2026-10-08-agentvm-mvp-design.md`

## Global Constraints

- Server su `127.0.0.1`, porta default `7777`.
- Dati in `~/AgentVMs/{images,golden,jobs}`; override con `AGENTVM_HOME`.
- Default: concorrenza 4, 4 vCPU, 4096 MB, timeout 1800 s.
- Token: Portachiavi `security find-generic-password -s agentvm -w`, fallback env `CLAUDE_CODE_OAUTH_TOKEN`. Mai in log, mai `set -x`.
- Branch risultato: `agent/<id>`, `<id>` = `YYYYMMDD-HHMMSS-xxxx` (4 hex casuali).
- Nessun mock/stub/fake: test su git, file, Portachiavi, VM e Claude veri. Trait solo con ≥2 implementazioni reali.
- Dipendenze: `http → app → domain`; `adapters` non si conoscono tra loro; `domain` senza I/O.
- Nessuna installazione sull'host oltre ai binari del progetto e alle crate Cargo.

## Review Focus

1. **Repo con modifiche non committate** → l'agente parte da `HEAD`, il working tree locale non viene toccato (test in Task 2: `fetch_bundle` non modifica file del working tree).
2. **Prompt con apici, newline, `$()`** → arriva intatto a Claude, nessuna esecuzione di shell (task.json via serde, `jq -r` nel guest; test in Task 1 sul round-trip di `TaskSpec` con prompt `"it's $(rm -rf /)\nok"`).
3. **Due client SSE sullo stesso task, uno connesso a metà** → entrambi ricevono la storia completa senza duplicati (test in Task 6 su `EventLog::subscribe`).
4. **Stop di un task ancora in coda** → passa a `stopped` senza avviare VM (test in Task 1 su `transition(Queued, StopRequested)`).
5. **Riga di `stream.jsonl` troncata o non JSON** → ignorata come `AgentEvent::Unparsed`, il follow continua (test in Task 1).

---

### Task 1: Scaffold + domain puro

**Files:**
- Create: `server/Cargo.toml`, `server/src/lib.rs`, `server/src/domain/{mod,ids,task,agent_event,outcome,spec}.rs`, `server/tests/fixtures/stream-hello.jsonl` (copiato da `~/AgentVMs/task1/job/stream.jsonl`, 0 token)
- Create: `.gitignore` (`/server/target`, `/bin`)

**Interfaces — Produces:**
- `ids`: `TaskId(String)` con `TaskId::generate(now: SystemTime, rand: [u8;2])` e `branch() -> String` (`agent/<id>`); `CommitSha::parse(&str) -> Result<CommitSha, IdError>` (40 hex); `RepoPath::new(PathBuf) -> Result<RepoPath, IdError>` (verifica solo che esista `<path>/.git`, nessun processo); `Prompt::new(String) -> Result<Prompt, IdError>` (trim non vuoto).
- `task`: `enum TaskState { Queued, Preparing, Booting, Running, Collecting, Done{branch:String, commits:u32}, NoChanges, Failed{reason:String}, Stopped }` (serde tag `"state"`, snake_case); `enum TaskEvent { SlotAcquired, Prepared, VmStarted, VmExited, Finished(Final), Failure(String), StopRequested }`; `fn transition(&TaskState, &TaskEvent) -> Result<TaskState, InvalidTransition>`; `TaskState::is_terminal()`.
- `outcome`: `struct GuestResult { status: GuestStatus /*ok|no_changes|failed*/, claude_exit: i32, commits: u32, error: Option<String> }` (Deserialize); `enum VmExit { Clean, Error(String), Signaled }`; `struct OutcomeInput { exit: VmExit, result: Option<GuestResult>, stop_requested: bool, timed_out: bool, has_out_bundle: bool }`; `enum Final { Done{commits:u32}, NoChanges, Failed(String), Stopped }`; `struct Outcome { final_: Final, fetch: bool }`; `fn decide(&OutcomeInput) -> Outcome`.
- `agent_event`: `enum AgentEvent { Init{model:String}, Text{text:String}, ToolUse{name:String, summary:String}, ToolResult{is_error:bool, summary:String}, Retry{attempt:u32}, Result{is_error:bool, duration_ms:u64, cost_usd:f64, text:String}, Unparsed{raw:String} }` (Serialize, tag `"kind"`); `fn parse_line(&str) -> Vec<AgentEvent>` (un messaggio assistant può contenere più blocchi). Summary troncati a 300 caratteri.
- `spec`: `struct TaskSpec { id, prompt, branch, base_sha, timeout_s }` (Serialize/Deserialize) = `task.json`.

Regole `decide` (ordine): stop → `Stopped`/no fetch; timeout → `Failed("timeout")`; `VmExit::Error(m)` → `Failed("vm_error: m")`; result `None` → `Failed("guest_no_result")`; `ok`+bundle → `Done`, fetch; `ok` senza bundle → `Failed("missing_out_bundle")`; `no_changes` → `NoChanges`; `failed` → `Failed(error.unwrap_or("claude_exit N"))`, fetch = `has_out_bundle`.

Transizioni valide: Queued→Preparing (SlotAcquired), Preparing→Booting (Prepared), Booting→Running (VmStarted), Running→Collecting (VmExited), Booting→Collecting (VmExited), Collecting→terminale (Finished), qualsiasi non terminale→Failed (Failure), Queued→Stopped (StopRequested); StopRequested sugli altri non terminali lascia lo stato invariato (lo stop effettivo arriva con Finished(Stopped)). Tutto il resto → `InvalidTransition`.

- [ ] Step 1: test unitari in ogni modulo: `transition` (tutti i casi sopra, incl. `Queued+StopRequested → Stopped`, `Done+VmStarted → Err`); `decide` (una prova per regola); `parse_line` sulle 6 righe della fixture reale → contiene `Init`, almeno un `ToolUse{name:"Write"|"Bash"}`, `Result{is_error:false}`; `parse_line("{\"type\":\"assi")` → `[Unparsed]`; round-trip serde `TaskSpec` col prompt `"it's $(rm -rf /)\nok"`; `TaskId::generate` formato e `branch()`.
- [ ] Step 2: `cargo test` → FAIL (moduli vuoti).
- [ ] Step 3: implementare.
- [ ] Step 4: `cargo test` → PASS.
- [ ] Step 5: commit `feat(domain): core puro (stati, esito, eventi agente)`.

### Task 2: Adapter host (git, jobdir, tail, keychain, secret)

**Files:**
- Create: `server/src/adapters/{mod,git,jobdir,tail,keychain}.rs`, `server/src/secret.rs`, `server/tests/adapters.rs`

**Interfaces — Produces:**
- `Secret` (`secret.rs`): `Secret::new(String)`, `expose(&self) -> &str`, `Debug` → `[REDACTED]`, nessun `Display`.
- `Git { repo: RepoPath }`: `rev_parse(&self, rev:&str) -> Result<CommitSha, GitError>`; `bundle_all(&self, dest:&Path)`; `fetch_bundle(&self, bundle:&Path, branch:&str)` (`git fetch <bundle> <branch>:<branch>`); `diff(&self, base:&CommitSha, branch:&str) -> Result<String, GitError>`; `commit_count(&self, base, branch) -> u32`. Tutto tramite `git -C <repo>`.
- `JobWorkspace` (`jobdir.rs`): `create(jobs_root:&Path, id:&TaskId) -> io::Result<Self>` crea `<root>/<id>/share` (0777); `dir()`, `share()`, `disk()`, `console()`, `efivars()`, `config_path()`, `pid_path()`; `write_spec(&TaskSpec)`, `write_token(&Secret)` (`share/.token`, 0600), `clone_disk(golden:&Path)` (`clonefile(2)` via `extern "C"`), `read_result() -> Option<GuestResult>` (JSON invalido → `None`), `has_out_bundle()`, `out_bundle()`; `Drop` cancella `disk.raw`, `efivars`, `share/.token`, `share/repo.bundle`. `fn cleanup_orphans(jobs_root:&Path)` legge `*/vm.pid`, termina il PID solo se `ps -p <pid> -o comm=` termina con `agentvm-vm`, cancella `disk.raw`.
- `tail_lines(path:PathBuf, stop: watch::Receiver<bool>) -> impl Stream<Item=String>` (`tail.rs`): poll 200 ms, attende che il file esista, emette righe complete, alla fine (stop=true) svuota il resto ed emette l'ultima riga anche senza `\n`.
- `Keychain { keychain: Option<PathBuf> }`: `read_token(&self) -> Result<Secret, KeychainError>` (`security find-generic-password -s agentvm -w [keychain]`; se fallisce, env `CLAUDE_CODE_OAUTH_TOKEN`; altrimenti `KeychainError::Missing` con messaggio che contiene `security add-generic-password -s agentvm -a agentvm -w`).

- [ ] Step 1: test in `tests/adapters.rs`, tutti con tempdir e strumenti veri:
  - `git_roundtrip`: repo temp con 1 commit + file non committato `dirty.txt`; `bundle_all`; `git clone` del bundle in un secondo dir, `checkout -b agent/x`, commit `new.txt`, `git bundle create out.bundle <base>..agent/x`; `fetch_bundle` → `rev_parse("agent/x")` ok, `commit_count == 1`, `diff` contiene `new.txt`, `dirty.txt` ancora presente e non committato.
  - `workspace_clone_and_drop`: file “golden” da 1 MiB → `clone_disk` → contenuto uguale; drop → `disk.raw` sparito, `share/stream.jsonl` scritto prima del drop ancora presente.
  - `workspace_token_perms`: `write_token` → modo `0o600`; `format!("{:?}", secret)` == `"[REDACTED]"`.
  - `read_result_invalid_json` → `None`.
  - `tail_follows_growing_file`: un processo `sh -c 'for i in 1 2 3; do echo l$i; sleep 0.3; done; printf tail'` scrive il file; lo stream produce `["l1","l2","l3","tail"]`.
  - `keychain_temp`: `security create-keychain -p x <tmp>/t.keychain-db`, `add-generic-password -s agentvm -a agentvm -w tok123 <kc>` → `read_token` == `tok123`; keychain vuoto e env assente → `Missing`.
- [ ] Step 2: `cargo test --test adapters` → FAIL.
- [ ] Step 3: implementare.
- [ ] Step 4: `cargo test` → PASS.
- [ ] Step 5: commit `feat(adapters): git, workspace, tail, keychain`.

### Task 3: Helper Swift `agentvm-vm` + build

**Files:**
- Create: `vm-helper/{main,Config,MachineFactory,Runner}.swift`, `vm-helper/vz.entitlements`, `scripts/build.sh`

**Interfaces — Produces:** `bin/agentvm-vm --config <job.json>`; protocollo §3.5 della spec (chiavi `disk, efivars, share, console, cpus, memory_mb, seed_iso`); eventi JSON Lines su stdout `started` / `stopped{seconds}` / `error{message}`; exit 0 / 1 / 130; SIGTERM e SIGINT → `vm.stop` → 130. Tag virtiofs `job`. Console virtio su file. NAT. EFI store creato se assente.
`scripts/build.sh`: `swiftc -O vm-helper/*.swift -o bin/agentvm-vm`, `codesign -s - -f --entitlements vm-helper/vz.entitlements bin/agentvm-vm`, `cargo build --release --manifest-path server/Cargo.toml`, copia `agentvm-server` in `bin/`.

- [ ] Step 1: `scripts/build.sh` → `bin/agentvm-vm` firmato (`codesign -d --entitlements -` mostra `com.apple.security.virtualization`).
- [ ] Step 2: `bin/agentvm-vm --config /nonexistent` → riga `{"event":"error",...}` ed exit 1.
- [ ] Step 3: commit `feat(vm-helper): processo VM Swift con protocollo JSON`.

### Task 4: Guest + immagine golden

**Files:**
- Create: `guest/agentvm-job`, `guest/agentvm.service`, `guest/10-agentvm.network`, `guest/setup-golden.sh`, `guest/golden-user-data.yaml`, `scripts/build-golden.sh`

**Interfaces:** `agentvm-job` implementa §3.6/§4 della spec: `set -euo pipefail`, nessun `set -x`; `trap` su EXIT che scrive `result.json {status:"failed", error:"guest_error"}` se non già scritto, poi `sync; poweroff -f`; senza `task.json` → `error:"no_task"`; token letto da `.token`, cancellato, passato con `sudo --preserve-env=CLAUDE_CODE_OAUTH_TOKEN`; git eseguito come `agent` con `user.name=agentvm`; attesa DNS `api.anthropic.com` max 10 s; auto-commit `agentvm: <id> (auto-commit)`; `out.bundle` solo se `commits>0`; status `failed` se `claude_exit≠0`, `no_changes` se 0 commit, altrimenti `ok`.
`build-golden.sh`: scarica e verifica `debian-13-genericcloud-arm64.tar.xz` (SHA512 da `SHA512SUMS`) solo se assente, clona il disco in `golden/disk.raw.tmp`, porta a 20 GiB, crea seed ISO (`hdiutil makehybrid -iso -joliet -default-volume-name cidata`), copia `guest/*` nella share, avvia `agentvm-vm` con `seed_iso`, verifica `GOLDEN_OK` in `setup.log`, rinomina in `golden/disk.raw`.

- [ ] Step 1: `scripts/build-golden.sh` → stampa `GOLDEN_OK`, crea `~/AgentVMs/golden/disk.raw`.
- [ ] Step 2: commit `feat(guest): job runner e build dell'immagine golden`.

### Task 5: Adapter `VmProcess` + test VM reale

**Files:**
- Create: `server/src/adapters/vm.rs`, `server/tests/vm.rs`

**Interfaces — Produces:** `VmConfig { disk, efivars, share, console, cpus:u32, memory_mb:u64, seed_iso:Option<PathBuf> }` (Serialize, chiavi §3.5); `VmProcess::spawn(helper:&Path, config_path:&Path, cfg:&VmConfig) -> Result<VmProcess, VmError>` (scrive il config, `kill_on_drop(true)`); `pid() -> u32`; `next_event(&mut self) -> Option<VmEvent>` con `enum VmEvent { Started, Stopped{seconds:f64}, Error(String) }`; `terminate(&self)` (SIGTERM via `extern "C" fn kill`); `wait(self) -> VmExit` (exit 0 → `Clean`, 130 → `Signaled`, altro → `Error(ultimo messaggio o "exit N")`).

- [ ] Step 1: test `#[ignore]` (richiedono golden + `bin/agentvm-vm`): `boots_golden_without_task` → `Started`, poi `wait()==Clean`, `result.json` con `error=="no_task"`, durata < 20 s; `terminate_running_vm` → dopo `Started` `terminate()` → `Signaled`.
- [ ] Step 2: `cargo test --test vm -- --ignored` → FAIL, poi implementare → PASS.
- [ ] Step 3: commit `feat(adapters): VmProcess`.

### Task 6: App (store, eventi, scheduler, supervisor)

**Files:**
- Create: `server/src/app/{mod,store,events,scheduler,supervisor}.rs`, `server/src/config.rs`

**Interfaces — Produces:**
- `Config { home, port:u16, concurrency:usize, cpus:u32, memory_mb:u64, timeout_s:u64, vm_helper:PathBuf }` con `Config::from_env()` (variabili `AGENTVM_HOME|PORT|CONCURRENCY|CPUS|MEMORY_MB|TIMEOUT_S|VM_HELPER`; helper default = `agentvm-vm` accanto all'eseguibile); `golden()`, `jobs()`.
- `events`: `enum StreamItem { State(TaskState), Agent(AgentEvent) }` (Serialize); `EventLog` con `push(StreamItem)` e `subscribe() -> (Vec<(u64,StreamItem)>, broadcast::Receiver<(u64,StreamItem)>)` atomico; il consumatore scarta seq ≤ ultimo della storia.
- `store`: `TaskRecord { id, repo, prompt, base_sha, state, created_at, finished_at }`; `Store` (Arc<Mutex<HashMap>> interno) con `insert`, `apply(&TaskId, TaskEvent) -> Result<TaskState, ..>` (usa `transition` e fa `push(State)`), `list()`, `get()`, `log(&TaskId) -> Arc<EventLog>`, `request_stop(&TaskId)`, `stop_signal(&TaskId) -> watch::Receiver<bool>`.
- `scheduler`: `Scheduler::new(concurrency)`; `acquire().await -> OwnedSemaphorePermit`.
- `supervisor`: `async fn run(ctx: Arc<AppCtx>, id: TaskId)` con `AppCtx { config, store, scheduler, keychain }`; `fn submit(ctx, repo, prompt, base_ref) -> Result<TaskId, SubmitError>` (valida `RepoPath`/`Prompt`, `rev_parse`, verifica token e golden *prima* di accodare, spawn di `run`). Flusso: permit → (stop? → `Finished(Stopped)`) → `SlotAcquired` → workspace, bundle, spec, token, clone → `Prepared` → spawn VM, `vm.pid` → `Started` → `VmStarted` + tail di `stream.jsonl` → `select!{ vm exit, stop, timeout(timeout_s) }` → `VmExited` → `decide` → fetch se richiesto (errore → `Failed("fetch_failed: …")`) → `Finished`. Errori di preparazione → `Failure(msg)`.

- [ ] Step 1: test unitari reali in `events.rs` (`subscribe` a metà: storia + live senza duplicati, due sottoscrittori) e in `store.rs` (apply fuori ordine → errore, stop in coda → `Stopped`). Il supervisor è coperto dal test di sistema (Task 7).
- [ ] Step 2: `cargo test` → FAIL, implementare → PASS.
- [ ] Step 3: commit `feat(app): store, eventi, scheduler e supervisor`.

### Task 7: HTTP, dashboard, main + test di sistema e avvio

**Files:**
- Create: `server/src/http/{mod,routes,dto}.rs`, `server/src/http/web/index.html`, `server/src/main.rs`, `server/tests/system.rs`, `docs/e2e.md`
- Modify: `README.md` (sezione "Uso")

**Interfaces:** route della spec §5 (sintassi axum `/{id}`), più `GET /api/status` → `{golden: bool, token: bool, concurrency, running}`. Errori JSON `{error: "..."}` con 400/404/409. SSE: evento `state` e `agent` (JSON di `StreamItem`), storia completa poi live, keep-alive 15 s. `main.rs`: `Config::from_env`, `cleanup_orphans`, bind `127.0.0.1:<port>`, log su stderr senza segreti. Dashboard: un file HTML con JS vanilla, form (repo, prompt, ref), elenco aggiornato ogni 2 s, dettaglio con log SSE e diff; avvisi se `golden=false` o `token=false` con il comando da eseguire; stato colorato; tema chiaro/scuro.

- [ ] Step 1: `server/tests/system.rs` `#[ignore]` (server, VM e Claude veri): avvia `bin/agentvm-server` su porta effimera con `AGENTVM_HOME` reale; repo temp con un commit; `POST /api/tasks` prompt "Crea hello.txt con scritto ciao e fai commit"; poll di `/api/tasks/{id}` fino a terminale (max 5 min) → `done`; `git -C repo show agent/<id>:hello.txt` contiene `ciao`; `/diff` contiene `hello.txt`; secondo task + `POST /stop` subito dopo `running` → `stopped`; grep `sk-ant-` su `~/AgentVMs/jobs` → 0. Tutto via `curl` e `git` CLI.
- [ ] Step 2: `scripts/build.sh && cargo test -- --ignored` → PASS.
- [ ] Step 3: 4 task in parallelo dalla dashboard → 4 `done`.
- [ ] Step 4: commit `feat(http): API, dashboard e test di sistema`.
- [ ] Step 5: avviare `bin/agentvm-server` e aprire `http://127.0.0.1:7777`.
