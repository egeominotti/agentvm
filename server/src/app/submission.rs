//! Task submission: validate a request, queue the task and hand it to its supervisor.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use super::context::AppCtx;
use super::launch;
use super::random::random_bytes;
use super::record::TaskRecord;
use crate::adapters::git::{Git, GitError};
use crate::adapters::keychain::KeychainError;
use crate::domain::ids::{IdError, Prompt, RepoPath, TaskId};
use crate::domain::settings::{ClaudeVersion, Model, SettingsError};
use crate::domain::snapshot::SnapshotId;

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
    Resources(#[from] SettingsError),
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

    let model = req.model.unwrap_or(ctx.settings.get().model);
    let id = TaskId::generate(SystemTime::now(), random_bytes());
    let mut record = TaskRecord::new(id, repo, prompt, base_sha, req.interactive).with_model(model);
    record.claude_version = req.claude_version.map(|v| v.as_str().to_owned());
    record.restore_from = req.restore_from;
    record.cpus = cpus;
    record.memory_mb = memory_mb;
    record.label = req.label;
    // Two launches in the same second may draw the same id: draw again, never replace a task.
    let id = loop {
        let id = record.id.clone();
        if !ctx.config.jobs().join(id.as_str()).exists() {
            match ctx.store.try_insert(record) {
                Ok(()) => break id,
                Err(r) => record = *r,
            }
        }
        record.id = TaskId::generate(SystemTime::now(), random_bytes());
    };
    tokio::spawn(launch::run(ctx.clone(), id.clone()));
    Ok(id)
}

fn expand_home(path: &str) -> PathBuf {
    match path.trim().strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest),
        None => PathBuf::from(path.trim()),
    }
}
