//! Operazioni git sul repo locale tramite la CLI `git`.

use std::path::Path;
use std::process::Command;

use crate::domain::ids::{CommitSha, IdError, RepoPath};

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("impossibile eseguire git: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("git {args} è fallito: {stderr}")]
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

    /// Importa `branch` da un bundle senza toccare working tree né branch corrente.
    pub fn fetch_bundle(&self, bundle: &Path, branch: &str) -> Result<(), GitError> {
        self.run(&["fetch", "--quiet", path_str(bundle), &format!("{branch}:{branch}")]).map(drop)
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
    p.to_str().expect("percorsi del progetto sempre UTF-8")
}
