//! Full task lifecycle: one supervisor (tokio task) per task.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use futures::StreamExt;
use tokio::sync::watch;

use super::events::StreamItem;
use super::golden::GoldenService;
use super::scheduler::Scheduler;
use super::settings::{SettingsService, UpdateError};
use super::store::{Store, TaskRecord};
use crate::adapters::git::{Git, GitError};
use crate::adapters::jobdir::JobWorkspace;
use crate::adapters::keychain::{Keychain, KeychainError};
use crate::adapters::releases::Releases;
use crate::adapters::snapshots::SnapshotStore;
use crate::adapters::tail::tail_lines;
use crate::adapters::vm::{VmConfig, VmEvent, VmProcess};
use crate::config::Config;
use crate::domain::agent_event::parse_line;
use crate::domain::ids::{IdError, Prompt, RepoPath, TaskId};
use crate::domain::outcome::{Final, OutcomeInput, VmExit, decide};
use crate::domain::settings::{ClaudeVersion, Model, Settings};
use crate::domain::snapshot::SnapshotId;
use crate::domain::spec::TaskSpec;
use crate::domain::task::{TaskEvent, TaskState};
use crate::guestfs;
use crate::secret::Secret;

const KILL_GRACE: Duration = Duration::from_secs(15);

/// Claude Code releases as served to the dashboard.
pub type ClaudeReleases = Releases;

/// Time the guest gets after its own time limit to save the work and power off.
const TIMEOUT_GRACE_S: u64 = 180;

pub struct AppCtx {
    pub config: Config,
    pub store: Store,
    pub scheduler: Scheduler,
    pub keychain: Keychain,
    pub settings: SettingsService,
    pub golden: GoldenService,
    pub snapshots: SnapshotStore,
    pub forwards: super::ports::PortForwards,
    releases: tokio::sync::Mutex<Option<(std::time::Instant, Releases)>>,
}

impl AppCtx {
    /// Settings saved in `AGENTVM_HOME` win over the environment defaults in `config`.
    pub fn new(config: Config, keychain: Keychain) -> Self {
        let limits = crate::adapters::host::host_limits();
        let defaults = Settings {
            max_vms: config.concurrency,
            cpus: config.cpus.min(limits.cpus),
            memory_mb: config.memory_mb,
            timeout_s: config.timeout_s,
            model: Model::default_choice(),
            default_repo: None,
            claude_version: Default::default(),
            s3: None,
            auto_snapshots: Default::default(),
        };
        let settings = SettingsService::load(config.home.join("settings.json"), defaults, limits);
        AppCtx {
            scheduler: Scheduler::new(settings.get().max_vms),
            store: Store::persistent(config.jobs()),
            golden: GoldenService::new(config.home.clone(), config.scripts_dir.join("build-golden.sh")),
            keychain,
            settings,
            snapshots: SnapshotStore::new(config.home.join("snapshots")),
            forwards: Default::default(),
            releases: tokio::sync::Mutex::new(None),
            config,
        }
    }

    /// Published Claude Code versions, cached for 10 minutes.
    pub async fn claude_releases(&self) -> Result<Releases, String> {
        let mut cache = self.releases.lock().await;
        if let Some((at, r)) = cache.as_ref()
            && at.elapsed() < Duration::from_secs(600)
        {
            return Ok(r.clone());
        }
        let fresh = tokio::task::spawn_blocking(crate::adapters::releases::fetch)
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        *cache = Some((std::time::Instant::now(), fresh.clone()));
        Ok(fresh)
    }

    pub fn update_settings(&self, new: Settings) -> Result<Settings, UpdateError> {
        let saved = self.settings.update(new)?;
        self.scheduler.resize(saved.max_vms);
        Ok(saved)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SubmitError {
    #[error(transparent)]
    Invalid(#[from] IdError),
    #[error("ref not found: {0}")]
    UnknownRef(#[source] GitError),
    #[error("golden image missing ({0}): run scripts/build-golden.sh")]
    NoGolden(String),
    #[error(transparent)]
    Token(#[from] KeychainError),
    #[error(transparent)]
    Resources(#[from] crate::domain::settings::SettingsError),
}

pub struct NewTask<'a> {
    pub repo: &'a str,
    pub prompt: String,
    pub base_ref: Option<&'a str>,
    /// Terminal with interactive Claude Code; the prompt becomes optional.
    pub interactive: bool,
    /// `None` uses the model from the settings.
    pub model: Option<Model>,
    /// `None` keeps the Claude Code version of the VM image.
    pub claude_version: Option<ClaudeVersion>,
    /// Boot from this snapshot instead of the golden image.
    pub restore_from: Option<SnapshotId>,
    /// vCPUs and memory for this VM; `None` uses the settings.
    pub cpus: Option<u32>,
    pub memory_mb: Option<u64>,
    /// Display name when there is no first task.
    pub label: Option<String>,
}

/// Validates the request, queues the task and starts its supervisor.
pub fn submit(ctx: &Arc<AppCtx>, req: NewTask<'_>) -> Result<TaskId, SubmitError> {
    let settings = ctx.settings.get();
    let (cpus, memory_mb) = (req.cpus.unwrap_or(settings.cpus), req.memory_mb.unwrap_or(settings.memory_mb));
    ctx.settings.limits().check_vm(cpus, memory_mb)?;
    let repo = RepoPath::new(expand_home(req.repo))?;
    let prompt = match Prompt::new(req.prompt) {
        Ok(p) => Some(p),
        Err(_) if req.interactive => None,
        Err(e) => return Err(e.into()),
    };
    let base_ref = req.base_ref.map(str::trim).filter(|r| !r.is_empty()).unwrap_or("HEAD");
    let base_sha = Git::new(repo.clone()).rev_parse(base_ref).map_err(SubmitError::UnknownRef)?;
    if req.restore_from.is_none() && !ctx.config.golden().is_file() {
        return Err(SubmitError::NoGolden(ctx.config.golden().display().to_string()));
    }
    ctx.keychain.read_token()?;

    let id = TaskId::generate(SystemTime::now(), random_bytes());
    let model = req.model.unwrap_or(ctx.settings.get().model);
    let mut record = TaskRecord::new(id.clone(), repo, prompt, base_sha, req.interactive).with_model(model);
    record.claude_version = req.claude_version.map(|v| v.as_str().to_owned());
    record.restore_from = req.restore_from;
    record.cpus = cpus;
    record.memory_mb = memory_mb;
    record.label = req.label;
    ctx.store.insert(record);
    tokio::spawn(run(ctx.clone(), id.clone()));
    Ok(id)
}

async fn run(ctx: Arc<AppCtx>, id: TaskId) {
    let Some(mut stop) = ctx.store.stop_signal(&id) else { return };
    let _permit = tokio::select! {
        permit = ctx.scheduler.acquire() => permit,
        _ = stop.wait_for(|s| *s) => return, // stopped while queued
    };
    if ctx.store.get(&id).is_none_or(|r| r.state != TaskState::Queued) {
        return;
    }
    if let Err(reason) = execute(&ctx, &id).await {
        let _ = ctx.store.apply(&id, TaskEvent::Failure(reason));
    }
}

/// Errors before the VM shuts down → `Err(reason)`; the normal outcome goes through `Finished`.
async fn execute(ctx: &AppCtx, id: &TaskId) -> Result<(), String> {
    let apply = |e| ctx.store.apply(id, e).map(drop).map_err(|e| e.to_string());
    apply(TaskEvent::SlotAcquired)?;
    let record = ctx.store.get(id).ok_or("task disappeared")?;
    let settings = ctx.settings.get();
    let branch = id.branch();
    let git = Git::new(record.repo.clone());

    let ws = JobWorkspace::create(&ctx.config.jobs(), id).map_err(|e| format!("job directory: {e}"))?;
    let t = std::time::Instant::now();
    if record.restore_from.is_none() {
        git.bundle_all(&ws.repo_bundle()).map_err(|e| e.to_string())?;
        ctx.store.push_boot(id, format!("host: repository packed in {} ms", t.elapsed().as_millis()));
    }
    ws.write_spec(&TaskSpec {
        id: id.to_string(),
        prompt: record.prompt.as_ref().map(|p| p.as_str().to_owned()).unwrap_or_default(),
        branch: branch.clone(),
        base_sha: record.base_sha.as_str().to_owned(),
        timeout_s: settings.timeout_s,
        interactive: record.interactive,
        model: record.model.cli_name().map(str::to_owned),
        claude_version: record.claude_version.clone(),
        restore: record.restore_from.is_some(),
    })
    .map_err(|e| format!("task.json: {e}"))?;
    let token = ctx.keychain.read_token().map_err(|e| e.to_string())?;
    ws.write_token(&token).map_err(|e| format!("token: {e}"))?;
    match &record.restore_from {
        Some(snap) => {
            ws.clone_disk(&ctx.snapshots.disk(snap)).map_err(|e| format!("snapshot disk clone: {e}"))?;
            ws.copy_efivars(&ctx.snapshots.efivars(snap)).map_err(|e| format!("snapshot EFI variables: {e}"))?;
        }
        None => ws.clone_disk(&ctx.config.golden()).map_err(|e| format!("disk clone: {e}"))?,
    }
    ctx.store.push_boot(id, format!("host: disk ready in {} ms", t.elapsed().as_millis()));
    apply(TaskEvent::Prepared)?;

    let stop_requested_early = ctx.store.stop_signal(id).is_some_and(|s| *s.borrow());
    if stop_requested_early {
        apply(TaskEvent::VmExited)?;
        return apply(TaskEvent::Finished(Final::Stopped, branch));
    }

    let vm = VmProcess::spawn(&ctx.config.vm_helper, &ws.config_path(), &vm_config(&record, &ws), &ws.events())
        .map_err(|e| e.to_string())?;
    let _ = ws.write_pid(vm.pid());
    supervise(ctx, id, &record, ws, vm, token).await
}

/// Follows a running VM until it stops, then collects the result. Shared by new VMs and by VMs
/// re-attached after a server restart.
async fn supervise(
    ctx: &AppCtx,
    id: &TaskId,
    record: &TaskRecord,
    ws: JobWorkspace,
    mut vm: VmProcess,
    token: Secret,
) -> Result<(), String> {
    let cost_sink = (ctx.store.clone_handle(), id.clone());
    let on_cost = move |cost_usd: f64| {
        let (store, id) = &cost_sink;
        let mut usage = store.get(id).and_then(|r| r.usage).unwrap_or_default();
        usage.cost_usd = cost_usd;
        store.set_usage(id, usage);
    };
    let follower = Follower::start(ctx.store.log(id).ok_or("task disappeared")?, ws.stream(), token, on_cost);
    let on_started = || {
        let _ = ctx.store.apply(id, TaskEvent::VmStarted);
    };
    let on_tick = || {
        if !ctx.store.get(id).is_some_and(|r| r.ready) {
            let lines = ws.job_log();
            let marker = if record.interactive { "terminal ready" } else { "network ready" };
            let ready = lines.iter().any(|l| l.ends_with(marker));
            ctx.store.set_guest_boot(id, lines, ready);
        }
        ctx.store.set_activity(id, ws.activity());
        if let Some(m) = ws.read_metrics() {
            if record.interactive {
                let socket = JobWorkspace::pty_socket_of(&ctx.config.jobs(), id);
                let vm = super::proxy::vm_name(record);
                let ports = ctx.forwards.sync(socket, id, &vm, ctx.config.port, &m.ports);
                ctx.store.set_ports(id, ports);
            }
            ctx.store.record_metrics(id, m);
        }
        if let Some(u) = ws.read_usage() {
            ctx.store.set_usage(id, u);
        }
    };
    // The guest enforces the limit itself and saves the work; this is the backstop if it cannot.
    let timeout = (!record.interactive).then(|| Duration::from_secs(ctx.settings.get().timeout_s + TIMEOUT_GRACE_S));
    let (exit, stop_requested, timed_out) = wait_for_vm(ctx, id, &mut vm, timeout, on_started, on_tick).await;
    ctx.store.set_activity(id, None);
    ctx.forwards.stop_all(id);
    ctx.store.set_ports(id, Vec::new());
    follower.finish().await;
    collect(ctx, id, record, &ws, exit, stop_requested, timed_out)
}

/// The VM is gone: decide the outcome, import the branch if needed, finish the task.
fn collect(
    ctx: &AppCtx,
    id: &TaskId,
    record: &TaskRecord,
    ws: &JobWorkspace,
    exit: VmExit,
    stop_requested: bool,
    timed_out: bool,
) -> Result<(), String> {
    let apply = |e| ctx.store.apply(id, e).map(drop).map_err(|e| e.to_string());
    if ctx.store.get(id).is_some_and(|r| r.state != TaskState::Collecting) {
        apply(TaskEvent::VmExited)?;
    }
    let exit = match exit {
        VmExit::Error(m) => VmExit::Error(with_console(m, &ws.console_tail(5))),
        other => other,
    };
    let outcome = decide(&OutcomeInput {
        exit,
        result: ws.read_result(),
        stop_requested,
        timed_out,
        has_out_bundle: ws.has_out_bundle(),
    });
    let mut branch = id.branch();
    let final_ = match (outcome.fetch, outcome.final_) {
        // Imported from a copy only the Mac controls, not from the file the guest could swap.
        (true, f) => match guestfs::copy_out(&ws.out_bundle(), &ws.dir().join("final.bundle"))
            .map_err(|e| e.to_string())
            .and_then(|_| {
                Git::new(record.repo.clone())
                    .import_bundle(&ws.dir().join("final.bundle"), &branch)
                    .map_err(|e| e.to_string())
            }) {
            Ok(landed) => {
                branch = landed;
                f
            }
            Err(e) => Final::Failed(format!("fetch_failed: {e}")),
        },
        (false, f) => f,
    };
    apply(TaskEvent::Finished(final_, branch))
}

/// After a restart: reload every task, re-attach to VMs that kept running, finish the others.
/// Returns the job ids whose VM is (still) owned by a task, so orphan cleanup leaves them alone.
pub fn recover(ctx: &Arc<AppCtx>) -> std::collections::HashSet<String> {
    let mut live = std::collections::HashSet::new();
    for record in Store::load(&ctx.config.jobs()) {
        let (id, state) = (record.id.clone(), record.state.clone());
        ctx.store.insert(record);
        match state {
            s if s.is_terminal() => {}
            TaskState::Queued => {
                tokio::spawn(run(ctx.clone(), id));
            }
            TaskState::Preparing => {
                let _ = ctx.store.apply(&id, TaskEvent::Failure("interrupted while preparing: launch it again".into()));
            }
            _ => {
                live.insert(id.to_string());
                tokio::spawn(resume(ctx.clone(), id));
            }
        }
    }
    live
}

async fn resume(ctx: Arc<AppCtx>, id: TaskId) {
    let _permit = ctx.scheduler.acquire().await;
    let Some(record) = ctx.store.get(&id) else { return };
    let ws = JobWorkspace::existing(&ctx.config.jobs(), &id);
    let vm = ws.read_pid().and_then(|pid| VmProcess::attach(pid, &ws.events()));
    let result = match vm {
        Some(vm) => {
            let token = ctx.keychain.read_token().unwrap_or_else(|_| Secret::new(String::new()));
            supervise(&ctx, &id, &record, ws, vm, token).await
        }
        None => {
            // The VM stopped while the server was down: whatever it left behind decides the outcome.
            let exit = if ws.read_result().is_some() {
                VmExit::Clean
            } else {
                VmExit::Error("the VM stopped while agentvm was not running".into())
            };
            collect(&ctx, &id, &record, &ws, exit, false, false)
        }
    };
    if let Err(reason) = result {
        let _ = ctx.store.apply(&id, TaskEvent::Failure(reason));
    }
}

/// Follows `stream.jsonl` and publishes the agent's events while the VM is alive.
struct Follower {
    stop: watch::Sender<bool>,
    handle: tokio::task::JoinHandle<()>,
}

impl Follower {
    /// Every line goes through `token.redact` before becoming a public event.
    fn start(
        log: Arc<super::events::EventLog>,
        stream: PathBuf,
        token: Secret,
        on_cost: impl Fn(f64) + Send + 'static,
    ) -> Self {
        let (stop, rx) = watch::channel(false);
        let handle = tokio::spawn(async move {
            let mut lines = std::pin::pin!(tail_lines(stream, rx));
            while let Some(line) = lines.next().await {
                for event in parse_line(&token.redact(&line)) {
                    if let crate::domain::agent_event::AgentEvent::Result { cost_usd, .. } = &event {
                        on_cost(*cost_usd);
                    }
                    log.push(StreamItem::Agent(event));
                }
            }
        });
        Follower { stop, handle }
    }

    /// Reads the last remaining lines and exits.
    async fn finish(self) {
        self.stop.send_replace(true);
        let _ = self.handle.await;
    }
}

/// Follows the VM from boot to shutdown. Stop and timeout also apply during boot;
/// if the helper ignores SIGTERM for `KILL_GRACE`, it gets SIGKILL. `on_tick` runs every second.
async fn wait_for_vm(
    ctx: &AppCtx,
    id: &TaskId,
    vm: &mut VmProcess,
    timeout: Option<Duration>,
    on_started: impl Fn(),
    on_tick: impl Fn(),
) -> (VmExit, bool, bool) {
    let (_never, fallback) = watch::channel(false);
    let mut stop = ctx.store.stop_signal(id).unwrap_or(fallback);
    let deadline = tokio::time::sleep(timeout.unwrap_or(Duration::MAX / 4));
    let kill_timer = tokio::time::sleep(Duration::MAX / 4);
    let mut ticks = tokio::time::interval(Duration::from_secs(1));
    tokio::pin!(deadline, kill_timer);
    let (mut stop_requested, mut timed_out) = (false, false);
    loop {
        let terminating = stop_requested || timed_out;
        tokio::select! {
            event = vm.next_event() => match event {
                Some(VmEvent::Started) => on_started(),
                Some(_) => {}
                None => break,
            },
            _ = stop.wait_for(|s| *s), if !terminating => stop_requested = true,
            _ = &mut deadline, if !terminating => timed_out = true,
            _ = &mut kill_timer, if terminating => vm.kill(),
            _ = ticks.tick() => on_tick(),
        }
        if !terminating && (stop_requested || timed_out) {
            vm.terminate();
            kill_timer.as_mut().reset(tokio::time::Instant::now() + KILL_GRACE);
        }
    }
    (vm.wait().await, stop_requested, timed_out)
}

fn vm_config(record: &TaskRecord, ws: &JobWorkspace) -> VmConfig {
    VmConfig {
        disk: ws.disk(),
        efivars: ws.efivars(),
        share: ws.share(),
        console: ws.console(),
        cpus: record.cpus,
        memory_mb: record.memory_mb,
        seed_iso: None,
        pty_socket: record.interactive.then(|| ws.pty_socket()),
    }
}

fn with_console(message: String, console_tail: &str) -> String {
    if console_tail.trim().is_empty() { message } else { format!("{message}\n--- console ---\n{console_tail}") }
}

fn expand_home(path: &str) -> PathBuf {
    match path.trim().strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest),
        None => PathBuf::from(path.trim()),
    }
}

pub fn random_bytes() -> [u8; 2] {
    use std::io::Read;
    let mut b = [0u8; 2];
    if std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).is_err() {
        let n = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        b = [(n >> 8) as u8, n as u8];
    }
    b
}
