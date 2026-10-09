//! A repository, checked before a VM is launched on it: does it exist, is it a git repository,
//! does it have a commit to start from.

use std::path::PathBuf;

use serde::Serialize;

use crate::adapters::git::Git;
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
}

pub fn check(path: &str) -> RepoCheck {
    let full = expand_home(path);
    let name = full.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut answer =
        RepoCheck { ok: false, path: full.display().to_string(), name, branch: None, sha: None, error: None };
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
