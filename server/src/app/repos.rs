//! A repository, checked before a VM is launched on it: does it exist, is it a git repository,
//! does it have a commit to start from. A link (GitHub…) is previewed without the network: where
//! it will be cloned, or the clone already there.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::adapters::git::Git;
use crate::domain::git_remote::GitRemote;
use crate::domain::ids::RepoPath;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct RepoCheck {
    /// A VM can be launched on it.
    pub ok: bool,
    /// The folder, with `~/` expanded.
    pub path: String,
    pub name: String,
    /// The branch HEAD is on (`None` when detached or when it is not a repository).
    pub branch: Option<String>,
    /// The commit a VM would start from.
    pub sha: Option<String>,
    /// Why a VM cannot be launched on it, in words for the user.
    pub error: Option<String>,
    /// Given as a link: what is cloned and fetched.
    pub remote: Option<String>,
    /// The link is cloned at launch (not on this Mac yet).
    pub to_clone: bool,
}

/// `input`: a folder, or a link to clone into `<home>/repos`.
pub fn check(home: &Path, input: &str) -> RepoCheck {
    if !GitRemote::looks_like_link(input) {
        return check_folder(input);
    }
    let remote = match GitRemote::parse(input) {
        Ok(r) => r,
        Err(e) => {
            let mut answer = check_folder(input);
            answer.ok = false;
            answer.error = Some(e);
            return answer;
        }
    };
    let dir = remote.dir(home);
    let mut answer = if dir.join(".git").is_dir() {
        check_folder(&dir.display().to_string())
    } else {
        let name = remote.path.rsplit('/').next().unwrap_or_default().to_owned();
        let path = dir.display().to_string();
        RepoCheck { ok: true, path, name, branch: None, sha: None, error: None, remote: None, to_clone: true }
    };
    answer.remote = Some(remote.url);
    answer
}

fn check_folder(path: &str) -> RepoCheck {
    let full = expand_home(path);
    let name = full.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut answer = RepoCheck {
        ok: false,
        path: full.display().to_string(),
        name,
        branch: None,
        sha: None,
        error: None,
        remote: None,
        to_clone: false,
    };
    if !full.is_dir() {
        answer.error = Some("This folder does not exist.".into());
        return answer;
    }
    let Ok(repo) = RepoPath::new(full) else {
        answer.error = Some("This folder is not a git repository (no .git inside).".into());
        return answer;
    };
    let git = Git::new(repo);
    answer.branch = git.current_branch();
    if git.is_shallow() {
        answer.error = Some(super::submission::SHALLOW.into());
        return answer;
    }
    match git.rev_parse("HEAD") {
        Ok(sha) => {
            answer.sha = Some(sha.as_str().to_owned());
            answer.ok = true;
        }
        Err(_) => answer.error = Some("This repository has no commits yet: commit something first.".into()),
    }
    answer
}

/// `~/code/app` is the user's home's `code/app`.
pub fn expand_home(path: &str) -> PathBuf {
    match path.trim().strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest),
        None => PathBuf::from(path.trim()),
    }
}
