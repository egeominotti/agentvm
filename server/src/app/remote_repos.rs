//! A repository given as a link: cloned the first time into `~/AgentVMs/repos/<host>/<path>`,
//! fetched before each launch, and the VM's branch pushed back to it. From there it is a
//! repository like any other: VMs start from it and their work lands in it as branches.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;

use super::context::AppCtx;
use crate::adapters::remote::{self, PROTOCOLS, Update};
use crate::domain::git_remote::GitRemote;
use crate::domain::ids::TaskId;

#[derive(Debug, thiserror::Error)]
pub enum RemoteRepoError {
    #[error("{0}")]
    Link(String),
    #[error("{0}")]
    Git(String),
    #[error(transparent)]
    DiskFull(#[from] crate::domain::disk::DiskFull),
    #[error("task not found")]
    NotFound,
}

/// The folder a link was opened into, ready for a launch.
#[derive(Debug, Clone)]
pub struct Opened {
    pub path: PathBuf,
    /// Cloned now (rather than brought up to date).
    pub cloned: bool,
    /// Something the user should know: the clone kept its own commits instead of updating.
    pub note: Option<String>,
}

/// Clones `link`, or fetches it when it was cloned before. Blocking (git over the network).
pub fn open(ctx: &AppCtx, link: &str) -> Result<Opened, RemoteRepoError> {
    let remote = GitRemote::parse(link).map_err(RemoteRepoError::Link)?;
    open_with(ctx, &remote, PROTOCOLS)
}

/// [`open`] for a link already read, with the transports git may use.
pub fn open_with(ctx: &AppCtx, remote: &GitRemote, protocols: &[&str]) -> Result<Opened, RemoteRepoError> {
    let path = remote.dir(&ctx.config.home);
    // One clone or fetch at a time per repository: launches of the same link wait for each other.
    let lock = {
        static LOCKS: Mutex<Option<HashMap<PathBuf, Arc<Mutex<()>>>>> = Mutex::new(None);
        LOCKS.lock().unwrap().get_or_insert_default().entry(path.clone()).or_default().clone()
    };
    let _held = lock.lock().unwrap_or_else(|e| e.into_inner());
    if path.join(".git").is_dir() {
        let note = match remote::update(&path, protocols).map_err(|e| RemoteRepoError::Git(e.to_string()))? {
            Update::Current => None,
            Update::Diverged(_) => Some(format!(
                "{} has commits or changes of its own: fetched, but VMs start from its own branch",
                path.display()
            )),
        };
        return Ok(Opened { path, cloned: false, note });
    }
    ctx.ensure_disk_space()?;
    remote::clone(&remote.url, &path, protocols).map_err(|e| RemoteRepoError::Git(e.to_string()))?;
    tracing::info!(url = %remote.url, path = %path.display(), "cloned");
    Ok(Opened { path, cloned: true, note: None })
}

/// A VM's branch, sent to its repository's origin.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct PushedBranch {
    pub branch: String,
    /// The remote it went to.
    pub remote: String,
    /// Where to open a pull request for it, on GitHub or GitLab.
    pub pull_request: Option<String>,
}

/// Pushes the VM's branch (its saved work) to the repository's `origin`. Blocking.
pub fn push(ctx: &AppCtx, id: &TaskId) -> Result<PushedBranch, RemoteRepoError> {
    let record = ctx.store.get(id).ok_or(RemoteRepoError::NotFound)?;
    let branch = record.branch();
    let pushed = remote::push(record.repo.as_path(), &branch).map_err(|e| {
        let text = e.to_string();
        if text.contains("src refspec") || text.contains("does not match any") {
            RemoteRepoError::Git(format!("Nothing to push yet: save the VM's work to {branch} first."))
        } else {
            RemoteRepoError::Git(text)
        }
    })?;
    tracing::info!(task = %id, branch = %branch, remote = %pushed.url, "pushed");
    let pull_request = GitRemote::parse(&pushed.url).ok().and_then(|r| r.pull_request(&branch));
    Ok(PushedBranch { branch, remote: pushed.url, pull_request })
}
