//! A repository given as a link: cloned the first time into `~/AgentVMs/repos/<host>/<path>`,
//! fetched before each launch, and the VM's branch pushed back to it. From there it is a
//! repository like any other: VMs start from it and their work lands in it as branches.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;

use super::context::AppCtx;
use crate::adapters::remote::{self, Auth, PROTOCOLS, Update, Visibility};
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
    #[error("{0}")]
    BadToken(String),
    #[error("{0}")]
    Keychain(String),
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
        let note = match remote::update(&path, protocols, auth_for(ctx, remote).as_ref())
            .map_err(|e| RemoteRepoError::Git(e.to_string()))?
        {
            Update::Current => None,
            Update::Diverged(_) => Some(format!(
                "{} has commits or changes of its own: fetched, but VMs start from its own branch",
                path.display()
            )),
        };
        return Ok(Opened { path, cloned: false, note });
    }
    ctx.ensure_disk_space()?;
    remote::clone(&remote.url, &path, protocols, auth_for(ctx, remote).as_ref())
        .map_err(|e| RemoteRepoError::Git(e.to_string()))?;
    tracing::info!(url = %remote.url, path = %path.display(), "cloned");
    Ok(Opened { path, cloned: true, note: None })
}

/// The token saved for the link's host, if any.
fn auth_for(ctx: &AppCtx, remote: &GitRemote) -> Option<Auth> {
    let token = ctx.keychain.read_git_token(&remote.host)?;
    Some(Auth { username: remote.token_user().to_owned(), token })
}

/// Who can read the repository behind `remote`, and whether this Mac (or a saved token) can.
pub fn visibility(ctx: &AppCtx, remote: &GitRemote) -> Visibility {
    remote::visibility(&remote.public_url(), &remote.url, PROTOCOLS, auth_for(ctx, remote).as_ref())
}

/// Hosts that may have a token: the usual ones, and every host agentvm has cloned from.
fn known_hosts(ctx: &AppCtx) -> Vec<String> {
    let mut hosts: Vec<String> = ["github.com", "gitlab.com", "bitbucket.org"].map(String::from).to_vec();
    if let Ok(dirs) = std::fs::read_dir(ctx.config.home.join("repos")) {
        hosts.extend(dirs.flatten().map(|d| d.file_name().to_string_lossy().into_owned()));
    }
    hosts.sort();
    hosts.dedup();
    hosts
}

/// The hosts with a token saved (the tokens never leave the Keychain this way).
pub fn token_hosts(ctx: &AppCtx) -> Vec<String> {
    let hosts = known_hosts(ctx);
    let refs: Vec<&str> = hosts.iter().map(String::as_str).collect();
    ctx.keychain.git_token_hosts(&refs).into_iter().map(String::from).collect()
}

/// Saves the token for `host` in the Keychain, for clones, fetches and pushes from this Mac.
pub fn save_token(ctx: &AppCtx, host: &str, token: crate::secret::Secret) -> Result<(), RemoteRepoError> {
    use crate::adapters::keychain::KeychainError;
    ctx.keychain.write_git_token(host, &token).map_err(|e| match e {
        KeychainError::InvalidGitToken => RemoteRepoError::BadToken(format!(
            "{e}: the host is a name like github.com, the token one line (e.g. github_pat_…)"
        )),
        other => RemoteRepoError::Keychain(other.to_string()),
    })
}

pub fn remove_token(ctx: &AppCtx, host: &str) -> Result<(), RemoteRepoError> {
    ctx.keychain.delete_git_token(host).map_err(|e| RemoteRepoError::BadToken(e.to_string()))
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
    // The token saved for origin's host, when origin is a link agentvm understands.
    let auth = remote::origin(record.repo.as_path())
        .and_then(|url| GitRemote::parse(&url).ok())
        .and_then(|r| auth_for(ctx, &r));
    let pushed = remote::push(record.repo.as_path(), &branch, auth.as_ref()).map_err(|e| {
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
