//! Git operations on the local repo via the `git` CLI.

use std::path::Path;
use std::process::Command;

use crate::domain::ids::{CommitSha, IdError, RepoPath};

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("cannot run git: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("git {args} failed: {stderr}")]
    Failed { args: String, stderr: String },
    #[error(transparent)]
    BadOutput(#[from] IdError),
}

pub struct Git {
    repo: RepoPath,
}

impl Git {
    pub fn new(repo: RepoPath) -> Self {
        Git { repo }
    }

    pub fn rev_parse(&self, rev: &str) -> Result<CommitSha, GitError> {
        let out = self.run(&["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")])?;
        Ok(CommitSha::parse(&out)?)
    }

    pub fn bundle_all(&self, dest: &Path) -> Result<(), GitError> {
        self.run(&["bundle", "create", "--quiet", path_str(dest), "--all"]).map(drop)
    }

    /// Imports `branch` from a bundle without touching any working tree, and returns the branch
    /// that received the work. `branch` moves only forward and only while no worktree has it
    /// checked out; otherwise (your own commits on it, or it is checked out) the work lands on
    /// `<branch>-vm` (or `-vm-2`, …) and nothing of yours is overwritten.
    pub fn import_bundle(&self, bundle: &Path, branch: &str) -> Result<String, GitError> {
        const INCOMING: &str = "refs/agentvm/incoming";
        self.run(&["fetch", "--quiet", "--no-write-fetch-head", path_str(bundle), &format!("+{branch}:{INCOMING}")])?;
        let result = self.rev_parse(INCOMING).and_then(|new| {
            let checked_out = self.run(&["worktree", "list", "--porcelain"])?;
            let candidates = std::iter::once(branch.to_owned())
                .chain(std::iter::once(format!("{branch}-vm")))
                .chain((2..100).map(|n| format!("{branch}-vm-{n}")));
            for name in candidates {
                let full = format!("refs/heads/{name}");
                if checked_out.lines().any(|l| l == format!("branch {full}")) {
                    continue;
                }
                match self.rev_parse(&full) {
                    Err(_) => {
                        // Create only if it still does not exist (empty old value).
                        self.run(&["update-ref", &full, new.as_str(), ""])?;
                        return Ok(name);
                    }
                    Ok(old) if old == new => return Ok(name),
                    Ok(old) if self.is_ancestor(&old, &new) => {
                        self.run(&["update-ref", &full, new.as_str(), old.as_str()])?;
                        return Ok(name);
                    }
                    Ok(_) => continue,
                }
            }
            Err(GitError::Failed { args: "import".into(), stderr: format!("no free branch name for {branch}") })
        });
        let _ = self.run(&["update-ref", "-d", INCOMING]);
        result
    }

    fn is_ancestor(&self, old: &CommitSha, new: &CommitSha) -> bool {
        self.run(&["merge-base", "--is-ancestor", old.as_str(), new.as_str()]).is_ok()
    }

    pub fn diff(&self, base: &CommitSha, branch: &str) -> Result<String, GitError> {
        self.run(&["diff", &format!("{}..{branch}", base.as_str())])
    }

    pub fn commit_count(&self, base: &CommitSha, branch: &str) -> Result<u32, GitError> {
        let out = self.run(&["rev-list", "--count", &format!("{}..{branch}", base.as_str())])?;
        Ok(out.trim().parse().unwrap_or(0))
    }

    fn run(&self, args: &[&str]) -> Result<String, GitError> {
        let out = Command::new("git").arg("-C").arg(self.repo.as_path()).args(args).output()?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(GitError::Failed {
                args: args.join(" "),
                stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
            })
        }
    }
}

fn path_str(p: &Path) -> &str {
    p.to_str().expect("project paths are always UTF-8")
}
