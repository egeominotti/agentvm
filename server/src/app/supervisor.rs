//! Ciclo di vita completo di un task: un supervisor (task tokio) per ogni task.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use futures::StreamExt;
use tokio::sync::watch;

use super::events::StreamItem;
use super::scheduler::Scheduler;
use super::store::{Store, TaskRecord};
use crate::adapters::git::{Git, GitError};
use crate::adapters::jobdir::JobWorkspace;
use crate::adapters::keychain::{Keychain, KeychainError};
use crate::adapters::tail::tail_lines;
use crate::adapters::vm::{VmConfig, VmEvent, VmProcess};
use crate::config::Config;
use crate::domain::agent_event::parse_line;
use crate::domain::ids::{IdError, Prompt, RepoPath, TaskId};
use crate::domain::outcome::{Final, OutcomeInput, VmExit, decide};
use crate::domain::spec::TaskSpec;
use crate::domain::task::{TaskEvent, TaskState};

pub struct AppCtx {
    pub config: Config,
    pub store: Store,
    pub scheduler: Scheduler,
    pub keychain: Keychain,
}

#[derive(Debug, thiserror::Error)]
pub enum SubmitError {
    #[error(transparent)]
    Invalid(#[from] IdError),
    #[error("ref non trovato: {0}")]
    UnknownRef(#[source] GitError),
    #[error("immagine golden assente ({0}): esegui scripts/build-golden.sh")]
    NoGolden(String),
    #[error(transparent)]
    Token(#[from] KeychainError),
}

/// Valida la richiesta, accoda il task e avvia il suo supervisor.
pub fn submit(ctx: &Arc<AppCtx>, repo: &str, prompt: String, base_ref: Option<&str>) -> Result<TaskId, SubmitError> {
    let repo = RepoPath::new(expand_home(repo))?;
    let prompt = Prompt::new(prompt)?;
    let base_ref = base_ref.map(str::trim).filter(|r| !r.is_empty()).unwrap_or("HEAD");
    let base_sha = Git::new(repo.clone()).rev_parse(base_ref).map_err(SubmitError::UnknownRef)?;
    if !ctx.config.golden().is_file() {
        return Err(SubmitError::NoGolden(ctx.config.golden().display().to_string()));
    }
    ctx.keychain.read_token()?;

    let id = TaskId::generate(SystemTime::now(), random_bytes());
    ctx.store.insert(TaskRecord::new(id.clone(), repo, prompt, base_sha));
    tokio::spawn(run(ctx.clone(), id.clone()));
    Ok(id)
}

async fn run(ctx: Arc<AppCtx>, id: TaskId) {
    let Some(mut stop) = ctx.store.stop_signal(&id) else { return };
    let _permit = tokio::select! {
        permit = ctx.scheduler.acquire() => permit,
        _ = stop.wait_for(|s| *s) => return, // fermato mentre era in coda
    };
    if ctx.store.get(&id).is_none_or(|r| r.state != TaskState::Queued) {
        return;
    }
    if let Err(reason) = execute(&ctx, &id).await {
        let _ = ctx.store.apply(&id, TaskEvent::Failure(reason));
    }
}

/// Errori prima dello spegnimento della VM → `Err(motivo)`; l'esito normale passa da `Finished`.
async fn execute(ctx: &AppCtx, id: &TaskId) -> Result<(), String> {
    let apply = |e| ctx.store.apply(id, e).map(drop).map_err(|e| e.to_string());
    apply(TaskEvent::SlotAcquired)?;
    let record = ctx.store.get(id).ok_or("task scomparso")?;
    let branch = id.branch();
    let git = Git::new(record.repo.clone());

    let ws = JobWorkspace::create(&ctx.config.jobs(), id).map_err(|e| format!("cartella del job: {e}"))?;
    git.bundle_all(&ws.repo_bundle()).map_err(|e| e.to_string())?;
    ws.write_spec(&TaskSpec {
        id: id.to_string(),
        prompt: record.prompt.as_str().to_owned(),
        branch: branch.clone(),
        base_sha: record.base_sha.as_str().to_owned(),
        timeout_s: ctx.config.timeout_s,
    })
    .map_err(|e| format!("task.json: {e}"))?;
    let token = ctx.keychain.read_token().map_err(|e| e.to_string())?;
    ws.write_token(&token).map_err(|e| format!("token: {e}"))?;
    drop(token);
    ws.clone_disk(&ctx.config.golden()).map_err(|e| format!("clone del disco: {e}"))?;
    apply(TaskEvent::Prepared)?;

    let mut vm = VmProcess::spawn(&ctx.config.vm_helper, &ws.config_path(), &vm_config(&ctx.config, &ws))
        .map_err(|e| e.to_string())?;
    let _ = ws.write_pid(vm.pid());
    if vm.next_event().await == Some(VmEvent::Started) {
        apply(TaskEvent::VmStarted)?;
    }

    let follower = Follower::start(ctx.store.log(id).ok_or("task scomparso")?, ws.stream());

    let (exit, stop_requested, timed_out) = wait_for_vm(ctx, id, &mut vm).await;
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

/// Segue `stream.jsonl` e pubblica gli eventi dell'agente finché la VM è viva.
struct Follower {
    stop: watch::Sender<bool>,
    handle: tokio::task::JoinHandle<()>,
}

impl Follower {
    fn start(log: Arc<super::events::EventLog>, stream: PathBuf) -> Self {
        let (stop, rx) = watch::channel(false);
        let handle = tokio::spawn(async move {
            let mut lines = std::pin::pin!(tail_lines(stream, rx));
            while let Some(line) = lines.next().await {
                for event in parse_line(&line) {
                    log.push(StreamItem::Agent(event));
                }
            }
        });
        Follower { stop, handle }
    }

    /// Legge le ultime righe rimaste e termina.
    async fn finish(self) {
        self.stop.send_replace(true);
        let _ = self.handle.await;
    }
}

/// Attende la fine della VM; su stop o timeout la ferma (una sola volta).
async fn wait_for_vm(ctx: &AppCtx, id: &TaskId, vm: &mut VmProcess) -> (VmExit, bool, bool) {
    let Some(mut stop) = ctx.store.stop_signal(id) else { return (vm.wait().await, false, false) };
    let deadline = tokio::time::sleep(Duration::from_secs(ctx.config.timeout_s));
    tokio::pin!(deadline);
    let (mut stop_requested, mut timed_out) = (false, false);
    loop {
        let terminating = stop_requested || timed_out;
        tokio::select! {
            exit = vm.wait() => return (exit, stop_requested, timed_out),
            _ = stop.wait_for(|s| *s), if !terminating => { stop_requested = true; vm.terminate(); }
            _ = &mut deadline, if !terminating => { timed_out = true; vm.terminate(); }
        }
    }
}

fn vm_config(config: &Config, ws: &JobWorkspace) -> VmConfig {
    VmConfig {
        disk: ws.disk(),
        efivars: ws.efivars(),
        share: ws.share(),
        console: ws.console(),
        cpus: config.cpus,
        memory_mb: config.memory_mb,
        seed_iso: None,
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
