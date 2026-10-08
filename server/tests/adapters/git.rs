//! Git: bundles in and out of the VM, without ever touching the user's working tree.

use std::path::Path;

use agentvm::adapters::git::Git;
use agentvm::domain::ids::RepoPath;

use crate::helpers::sh;

fn new_repo(dir: &Path) {
    sh(
        dir,
        "git init -q -b main && git config user.email t@t && git config user.name t \
             && echo base > base.txt && git add . && git commit -qm base",
    );
}

#[test]
fn git_roundtrip_through_bundles() {
    let tmp = tempfile::tempdir().unwrap();
    let repo_dir = tmp.path().join("repo");
    std::fs::create_dir(&repo_dir).unwrap();
    new_repo(&repo_dir);
    std::fs::write(repo_dir.join("dirty.txt"), "uncommitted").unwrap();

    let git = Git::new(RepoPath::new(repo_dir.clone()).unwrap());
    let base = git.rev_parse("HEAD").unwrap();
    let bundle = tmp.path().join("repo.bundle");
    git.bundle_all(&bundle).unwrap();

    // The "guest": clones from the bundle, works on a branch and produces out.bundle.
    let work = tmp.path().join("work");
    sh(tmp.path(), &format!("git clone -q {} work", bundle.display()));
    sh(
        &work,
        &format!(
            "git config user.email a@a && git config user.name a && git checkout -q -b agent/x {} \
         && echo new > new.txt && git add . && git commit -qm new \
         && git bundle create -q ../out.bundle {}..agent/x",
            base.as_str(),
            base.as_str()
        ),
    );

    assert_eq!(git.import_bundle(&tmp.path().join("out.bundle"), "agent/x").unwrap(), "agent/x");
    assert!(git.rev_parse("agent/x").is_ok());
    assert_eq!(git.commit_count(&base, "agent/x").unwrap(), 1);
    assert!(git.diff(&base, "agent/x").unwrap().contains("new.txt"));
    // The local working tree was not touched.
    assert_eq!(std::fs::read_to_string(repo_dir.join("dirty.txt")).unwrap(), "uncommitted");
    assert!(sh(&repo_dir, "git status --porcelain").contains("?? dirty.txt"));
    assert!(!repo_dir.join("new.txt").exists());
}

/// A repo plus a "guest" clone of it working on agent/x; returns (git, repo dir, guest dir).
fn repo_and_guest(tmp: &Path) -> (Git, std::path::PathBuf, std::path::PathBuf) {
    let repo_dir = tmp.join("repo");
    std::fs::create_dir(&repo_dir).unwrap();
    new_repo(&repo_dir);
    let git = Git::new(RepoPath::new(repo_dir.clone()).unwrap());
    git.bundle_all(&tmp.join("repo.bundle")).unwrap();
    sh(tmp, "git clone -q repo.bundle guest");
    let guest = tmp.join("guest");
    sh(&guest, "git config user.email a@a && git config user.name a && git checkout -q -b agent/x");
    (git, repo_dir, guest)
}

/// The guest commits `file` and bundles its branch, as `save_work` does.
fn guest_saves(guest: &Path, file: &str) -> std::path::PathBuf {
    sh(
        guest,
        &format!(
            "echo {file} > {file} && git add . && git commit -qm {file} && git bundle create -q ../out.bundle main..agent/x"
        ),
    );
    guest.parent().unwrap().join("out.bundle")
}

#[test]
fn importing_never_discards_commits_made_on_the_agent_branch_by_hand() {
    let tmp = tempfile::tempdir().unwrap();
    let (git, repo, guest) = repo_and_guest(tmp.path());
    assert_eq!(git.import_bundle(&guest_saves(&guest, "a.txt"), "agent/x").unwrap(), "agent/x");
    sh(
        &repo,
        "git checkout -q agent/x && echo mine > mine.txt && git add . && git commit -qm mine && git checkout -q main",
    );

    let landed = git.import_bundle(&guest_saves(&guest, "b.txt"), "agent/x").unwrap();
    assert_eq!(landed, "agent/x-vm");
    assert!(sh(&repo, "git show agent/x:mine.txt").contains("mine"), "the hand-made commit was lost");
    assert!(sh(&repo, "git show agent/x-vm:b.txt").contains("b.txt"));
}

#[test]
fn importing_while_the_agent_branch_is_checked_out_leaves_the_worktree_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let (git, repo, guest) = repo_and_guest(tmp.path());
    git.import_bundle(&guest_saves(&guest, "a.txt"), "agent/x").unwrap();
    sh(&repo, "git checkout -q agent/x");

    let landed = git.import_bundle(&guest_saves(&guest, "b.txt"), "agent/x").unwrap();
    assert_eq!(landed, "agent/x-vm");
    assert_eq!(sh(&repo, "git status --porcelain"), "", "the worktree no longer matches its branch");
    assert!(sh(&repo, "git show agent/x-vm:b.txt").contains("b.txt"));
    // Later saves keep updating the same side branch.
    assert_eq!(git.import_bundle(&guest_saves(&guest, "c.txt"), "agent/x").unwrap(), "agent/x-vm");
    assert!(sh(&repo, "git show agent/x-vm:c.txt").contains("c.txt"));
}

#[test]
fn git_rev_parse_unknown_ref_fails() {
    let tmp = tempfile::tempdir().unwrap();
    new_repo(tmp.path());
    let git = Git::new(RepoPath::new(tmp.path().to_path_buf()).unwrap());
    assert!(git.rev_parse("does-not-exist").is_err());
}
