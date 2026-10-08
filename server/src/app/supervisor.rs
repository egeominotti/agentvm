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
use crate::adapters::tail::tail_lines;
use crate::adapters::vm::{VmConfig, VmEvent, VmProcess};
use crate::secret::Secret;
use crate::config::Config;
use crate::domain::agent_event::parse_line;
use crate::domain::ids::{IdError, Prompt, RepoPath, TaskId};
use crate::domain::outcome::{Final, OutcomeInput, VmExit, decide};
use crate::domain::settings::{Model, Settings};
use crate::domain::spec::TaskSpec;
use crate::domain::task::{TaskEvent, TaskState};

const KILL_GRACE: Duration = Duration::from_secs(15);

pub struct AppCtx {
    pub config: Config,
    pub store: Store,
    pub scheduler: Scheduler,
    pub keychain: Keychain,
    pub settings: SettingsService,
    pub golden: GoldenService,
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
            model: Model::Default,
            default_repo: None,
        };
        let settings = SettingsService::load(config.home.join("settings.json"), defaults, limits);
        AppCtx {
            scheduler: Scheduler::new(settings.get().max_vms),
            store: Store::new(),
            golden: GoldenService::new(config.home.clone(), config.scripts_dir.join("build-golden.sh")),
            keychain,
            settings,
            config,
        }
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
}

pub struct NewTask<'a> {
    pub repo: &'a str,
    pub prompt: String,
    pub base_ref: Option<&'a str>,
    /// Terminal with interactive Claude Code; the prompt becomes optional.
    pub interactive: bool,
    /// `None` uses the model from the settings.
    pub model: Option<Model>,
}

/// Validates the request, queues the task and starts its supervisor.
pub fn submit(ctx: &Arc<AppCtx>, req: NewTask<'_>) -> Result<TaskId, SubmitError> {
    let repo = RepoPath::new(expand_home(req.repo))?;
    let prompt = match Prompt::new(req.prompt) {
        Ok(p) => Some(p),
        Err(_) if req.interactive => None,
        Err(e) => return Err(e.into()),
    };
    let base_ref = req.base_ref.map(str::trim).filter(|r| !r.is_empty()).unwrap_or("HEAD");
    let base_sha = Git::new(repo.clone()).rev_parse(base_ref).map_err(SubmitError::UnknownRef)?;
    if !ctx.config.golden().is_file() {
        return Err(SubmitError::NoGolden(ctx.config.golden().display().to_string()));
    }
    ctx.keychain.read_token()?;

    let id = TaskId::generate(SystemTime::now(), random_bytes());
    let model = req.model.unwrap_or(ctx.settings.get().model);
    ctx.store.insert(TaskRecord::new(id.clone(), repo, prompt, base_sha, req.interactive).with_model(model));
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
    git.bundle_all(&ws.repo_bundle()).map_err(|e| e.to_string())?;
    ws.write_spec(&TaskSpec {
        id: id.to_string(),
        prompt: record.prompt.as_ref().map(|p| p.as_str().to_owned()).unwrap_or_default(),
        branch: branch.clone(),
        base_sha: record.base_sha.as_str().to_owned(),
        timeout_s: settings.timeout_s,
        interactive: record.interactive,
        model: record.model.cli_name().map(str::to_owned),
    })
    .map_err(|e| format!("task.json: {e}"))?;
    let token = ctx.keychain.read_token().map_err(|e| e.to_string())?;
    ws.write_token(&token).map_err(|e| format!("token: {e}"))?;
    ws.clone_disk(&ctx.config.golden()).map_err(|e| format!("disk clone: {e}"))?;
    apply(TaskEvent::Prepared)?;

    let stop_requested_early = ctx.store.stop_signal(id).is_some_and(|s| *s.borrow());
    if stop_requested_early {
        apply(TaskEvent::VmExited)?;
        return apply(TaskEvent::Finished(Final::Stopped, branch));
    }

    let mut vm = VmProcess::spawn(&ctx.config.vm_helper, &ws.config_path(), &vm_config(&settings, &ws, record.interactive))
        .map_err(|e| e.to_string())?;
    let _ = ws.write_pid(vm.pid());
    let follower = Follower::start(ctx.store.log(id).ok_or("task disappeared")?, ws.stream(), token);

    let on_started = || {
        let _ = ctx.store.apply(id, TaskEvent::VmStarted);
    };
    let on_tick = || {
        ctx.store.set_activity(id, ws.activity());
        if let Some(m) = ws.read_metrics() {
            ctx.store.record_metrics(id, m);
        }
    };
    let timeout = (!record.interactive).then(|| Duration::from_secs(settings.timeout_s));
    let (exit, stop_requested, timed_out) = wait_for_vm(ctx, id, &mut vm, timeout, on_started, on_tick).await;
    ctx.store.set_activity(id, None);
    apply(TaskEvent::VmExited)?;
    follower.finish().await;

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
    let final_ = match (outcome.fetch, outcome.final_) {
        (true, f) => match git.fetch_bundle(&ws.out_bundle(), &branch) {
            Ok(()) => f,
            Err(e) => Final::Failed(format!("fetch_failed: {e}")),
        },
        (false, f) => f,
    };
    apply(TaskEvent::Finished(final_, branch))
}

/// Follows `stream.jsonl` and publishes the agent's events while the VM is alive.
struct Follower {
    stop: watch::Sender<bool>,
    handle: tokio::task::JoinHandle<()>,
}

impl Follower {
    /// Every line goes through `token.redact` before becoming a public event.
    fn start(log: Arc<super::events::EventLog>, stream: PathBuf, token: Secret) -> Self {
        let (stop, rx) = watch::channel(false);
        let handle = tokio::spawn(async move {
            let mut lines = std::pin::pin!(tail_lines(stream, rx));
            while let Some(line) = lines.next().await {
                for event in parse_line(&token.redact(&line)) {
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

fn vm_config(settings: &Settings, ws: &JobWorkspace, interactive: bool) -> VmConfig {
    VmConfig {
        disk: ws.disk(),
        efivars: ws.efivars(),
        share: ws.share(),
        console: ws.console(),
        cpus: settings.cpus,
        memory_mb: settings.memory_mb,
        seed_iso: None,
        pty_socket: interactive.then(|| ws.pty_socket()),
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

fn random_bytes() -> [u8; 2] {
    use std::io::Read;
    let mut b = [0u8; 2];
    if std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).is_err() {
        let n = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        b = [(n >> 8) as u8, n as u8];
    }
    b
}
