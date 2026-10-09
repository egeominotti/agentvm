//! A repository, checked before a VM is launched on it: does it exist, is it a git repository,
//! does it have a commit to start from, which branches can it start from.

use std::path::PathBuf;

use serde::Serialize;

use super::context::AppCtx;
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
    /// For a link: `public`, `private` (this Mac or a saved token can read it) or `no_access`.
    pub visibility: Option<String>,
    /// Worth knowing, without stopping a launch (the remote out of reach, the copy here is used).
    pub warning: Option<String>,
    /// What a VM can start from, the default first.
    pub branches: Vec<String>,
    /// The branch a launch starts from unless another is chosen.
    pub default_branch: Option<String>,
}

/// `input`: a folder, or a link to clone into `<home>/repos` (told public or private over the
/// network, with the Mac's access or a saved token: see `repo_link`).
pub fn check(ctx: &AppCtx, input: &str) -> RepoCheck {
    if GitRemote::looks_like_link(input) {
        return super::repo_link::check_link(ctx, input);
    }
    check_folder(input)
}

pub(super) fn check_folder(path: &str) -> RepoCheck {
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
        visibility: None,
        warning: None,
        branches: Vec::new(),
        default_branch: None,
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
    answer.default_branch = answer.branch.clone();
    answer.branches = git.branches();
    // The checked-out branch first: it is what a launch starts from by default.
    if let Some(current) = &answer.branch
        && let Some(i) = answer.branches.iter().position(|b| b == current)
    {
        let b = answer.branches.remove(i);
        answer.branches.insert(0, b);
    }
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
