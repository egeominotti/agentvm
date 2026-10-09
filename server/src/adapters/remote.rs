//! Repositories that live elsewhere (GitHub, GitLab…): told public or private, cloned into
//! agentvm's own folder, brought up to date before a launch, and the VM's branch pushed back. Git
//! runs on the Mac with the user's access (ssh key, credential helper) or a token saved in agentvm,
//! never asking for a password: it fails instead. A token reaches git only through the
//! environment of that one git process: never a command line, a URL or a file.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// What a link from the dashboard may use: no file://, no ext:: (which runs commands).
pub const PROTOCOLS: &[&str] = &["https", "ssh"];

/// A first clone of a big repository over a slow link takes long; a stuck one must still end.
const LIMIT: std::time::Duration = std::time::Duration::from_secs(30 * 60);

#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error("cannot run git: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("{0}")]
    Failed(String),
}

/// What an update did to the clone's checked-out branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
    /// It is at the remote's latest commit.
    Current,
    /// It has commits of its own, or changes: fetched, but left where it was (the reason).
    Diverged(String),
}

/// A token for a private repository: given to git as the password for `username`.
pub struct Auth {
    pub username: String,
    pub token: crate::secret::Secret,
}

/// Who can read a repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visibility {
    /// Anyone, without credentials.
    Public,
    /// Not anonymously, but with this Mac's access or the token.
    Private,
    /// Private (or missing), and no access works: git's words.
    NoAccess(String),
}

/// Public, private with access, or out of reach. `public_url`: the https form, tried with no
/// credentials at all; `url`: the link itself, tried with the Mac's access or `auth`.
pub fn visibility(public_url: &str, url: &str, protocols: &[&str], auth: Option<&Auth>) -> Visibility {
    let mut anonymous = git(None, protocols, None);
    // No credential helper at all (macOS sets one system-wide): only what anyone can read.
    anonymous.args(["-c", "credential.helper=", "ls-remote", "--heads", "--"]).arg(public_url);
    if run_limited(&mut anonymous, "ls-remote", PROBE_LIMIT).is_ok() {
        return Visibility::Public;
    }
    let mut known = git(None, protocols, auth);
    known.args(["ls-remote", "--heads", "--"]).arg(url);
    match run_limited(&mut known, "ls-remote", PROBE_LIMIT) {
        Ok(_) => Visibility::Private,
        Err(e) => Visibility::NoAccess(e.to_string()),
    }
}

/// Telling public from private must not hold the New VM dialog for long.
const PROBE_LIMIT: std::time::Duration = std::time::Duration::from_secs(20);

/// A branch sent to the repository's `origin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pushed {
    pub url: String,
    pub branch: String,
}

/// Clones `url` whole into `dest`, which appears only once complete: a failed or interrupted
/// clone leaves nothing behind. `protocols`: the transports git may use ([`PROTOCOLS`]).
pub fn clone(url: &str, dest: &Path, protocols: &[&str], auth: Option<&Auth>) -> Result<(), RemoteError> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = dest.parent().ok_or_else(|| RemoteError::Failed("no folder for the clone".into()))?;
    std::fs::create_dir_all(parent)?;
    if dest.exists() {
        return Err(RemoteError::Failed(format!("{} already exists", dest.display())));
    }
    let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = parent.join(format!(".{name}.cloning-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let mut cmd = git(None, protocols, auth);
    cmd.args(["clone", "--quiet", "--no-recurse-submodules", "--"]).arg(url).arg(&tmp);
    let done = run(&mut cmd, "clone").and_then(|_| std::fs::rename(&tmp, dest).map_err(RemoteError::from));
    if done.is_err() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    done
}

/// Fetches the remote, then moves the checked-out branch to its upstream when that only adds
/// commits. Local commits or changes are never rewritten: the branch is then left as it was.
pub fn update(repo: &Path, protocols: &[&str], auth: Option<&Auth>) -> Result<Update, RemoteError> {
    run(git(Some(repo), protocols, auth).args(["fetch", "--quiet", "--prune", "origin"]), "fetch")?;
    match run(git(Some(repo), protocols, None).args(["merge", "--ff-only", "--quiet", "@{upstream}"]), "update") {
        Ok(_) => Ok(Update::Current),
        Err(RemoteError::Failed(why)) => Ok(Update::Diverged(why)),
        Err(e) => Err(e),
    }
}

/// The URL of the repository's `origin`, if it has one.
pub fn origin(repo: &Path) -> Option<String> {
    let url = run(git(Some(repo), &[], None).args(["config", "--get", "remote.origin.url"]), "read origin").ok()?;
    Some(url.trim().to_owned()).filter(|u| !u.is_empty())
}

/// Pushes `branch` to the same branch on `origin` (never forced).
pub fn push(repo: &Path, branch: &str, auth: Option<&Auth>) -> Result<Pushed, RemoteError> {
    let url = run(git(Some(repo), &[], None).args(["config", "--get", "remote.origin.url"]), "read origin")
        .map_err(|_| RemoteError::Failed("This repository has no remote named origin to push to.".into()))?;
    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
    // The user's own origin: whatever transport it uses.
    let mut cmd = git(Some(repo), &[], auth);
    cmd.args(["push", "--quiet", "--porcelain", "origin", &refspec]);
    run(&mut cmd, "push")?;
    Ok(Pushed { url: url.trim().to_owned(), branch: branch.to_owned() })
}

/// Git that never prompts (a missing credential fails at once) and runs no hooks of ours. With
/// `auth`, the only credential it uses is that token: a helper reads it from this git's own
/// environment, so it never shows on a command line, in a URL or in a file.
fn git(dir: Option<&Path>, protocols: &[&str], auth: Option<&Auth>) -> Command {
    let mut cmd = Command::new("git");
    if let Some(dir) = dir {
        cmd.arg("-C").arg(dir);
    }
    cmd.args(["-c", "core.hooksPath=/dev/null"]);
    if !protocols.is_empty() {
        cmd.args(["-c", "protocol.allow=never"]);
        for p in protocols {
            cmd.arg("-c").arg(format!("protocol.{p}.allow=always"));
        }
    }
    if let Some(auth) = auth {
        cmd.args(["-c", "credential.helper="]);
        cmd.args([
            "-c",
            r#"credential.helper=!f() { test "$1" = get && printf 'username=%s\npassword=%s\n' "$AGENTVM_GIT_USER" "$AGENTVM_GIT_TOKEN"; }; f"#,
        ]);
        cmd.env("AGENTVM_GIT_USER", &auth.username).env("AGENTVM_GIT_TOKEN", auth.token.expose());
    }
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    if std::env::var_os("GIT_SSH_COMMAND").is_none() {
        cmd.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes");
    }
    cmd
}

fn run(cmd: &mut Command, what: &str) -> Result<String, RemoteError> {
    run_limited(cmd, what, LIMIT)
}

fn run_limited(cmd: &mut Command, what: &str, limit: std::time::Duration) -> Result<String, RemoteError> {
    let out = crate::process::output(cmd, limit)?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
    Err(RemoteError::Failed(format!("git {what} failed: {}", explain(&stderr))))
}

/// Git's own words, plus what to do when it is about access.
fn explain(stderr: &str) -> String {
    let lower = stderr.to_lowercase();
    if lower.contains("could not read username")
        || lower.contains("authentication failed")
        || lower.contains("permission denied (publickey)")
        || lower.contains("repository not found")
    {
        format!(
            "{stderr}. No access from this Mac: add a token in Settings › Git access, or sign in with `gh auth login`, or for ssh add your key to the host."
        )
    } else {
        stderr.to_owned()
    }
}
