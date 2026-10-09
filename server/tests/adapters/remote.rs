//! Repositories from a link: cloned whole into agentvm's folder, kept up to date, and the VM's
//! branch pushed back. Against real repositories, with a local bare one as the remote.

use std::path::Path;

use agentvm::adapters::remote::{self, Update};

use crate::helpers::sh;

/// The test remote is a local bare repository: the file protocol is allowed for it here only.
const TEST: &[&str] = &["file"];

fn remote_with_commit(root: &Path) -> String {
    let src = root.join("src");
    std::fs::create_dir(&src).unwrap();
    sh(
        &src,
        "git init -q -b main && git config user.email t@t && git config user.name t && echo one > a.txt && git add . && git commit -qm one",
    );
    sh(root, "git clone -q --bare src remote.git");
    format!("file://{}", root.join("remote.git").display())
}

#[test]
fn a_clone_is_whole_and_appears_only_when_complete() {
    let tmp = tempfile::tempdir().unwrap();
    let url = remote_with_commit(tmp.path());
    let dest = tmp.path().join("repos/host/acme/shop");
    remote::clone(&url, &dest, TEST, None).unwrap();
    assert_eq!(sh(&dest, "git log --format=%s"), "one\n");
    assert_eq!(sh(&dest, "git rev-parse --is-shallow-repository"), "false\n");
    // Nothing left beside it from the clone in progress.
    let siblings: Vec<_> =
        std::fs::read_dir(dest.parent().unwrap()).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(siblings, vec![std::ffi::OsString::from("shop")]);
}

#[test]
fn a_failed_clone_leaves_nothing_behind() {
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("repos/host/acme/missing");
    let missing = format!("file://{}", tmp.path().join("nothing.git").display());
    assert!(remote::clone(&missing, &dest, TEST, None).is_err());
    assert!(!dest.exists());
    let left = std::fs::read_dir(dest.parent().unwrap()).map(|d| d.count()).unwrap_or(0);
    assert_eq!(left, 0, "a partial clone was left");
}

/// Only the protocols allowed get through: by default a file:// link (a folder of the Mac) is refused.
#[test]
fn protocols_not_allowed_are_refused_by_git_itself() {
    let tmp = tempfile::tempdir().unwrap();
    let url = remote_with_commit(tmp.path());
    let err = remote::clone(&url, &tmp.path().join("x"), remote::PROTOCOLS, None).unwrap_err().to_string();
    assert!(err.contains("not allowed") || err.contains("transport"), "{err}");
}

#[test]
fn an_update_brings_the_remote_s_new_commits() {
    let tmp = tempfile::tempdir().unwrap();
    let url = remote_with_commit(tmp.path());
    let dest = tmp.path().join("clone");
    remote::clone(&url, &dest, TEST, None).unwrap();
    sh(
        &tmp.path().join("src"),
        "echo two > b.txt && git add . && git commit -qm two && git push -q ../remote.git main",
    );
    assert_eq!(remote::update(&dest, TEST, None).unwrap(), Update::Current);
    assert_eq!(sh(&dest, "git log -1 --format=%s"), "two\n");
}

/// A clone where someone committed on the default branch is fetched but not moved: nothing lost.
#[test]
fn an_update_never_rewrites_local_work() {
    let tmp = tempfile::tempdir().unwrap();
    let url = remote_with_commit(tmp.path());
    let dest = tmp.path().join("clone");
    remote::clone(&url, &dest, TEST, None).unwrap();
    sh(
        &dest,
        "git config user.email l@l && git config user.name l && echo mine > m.txt && git add . && git commit -qm mine",
    );
    sh(
        &tmp.path().join("src"),
        "echo two > b.txt && git add . && git commit -qm two && git push -q ../remote.git main",
    );
    assert!(matches!(remote::update(&dest, TEST, None).unwrap(), Update::Diverged(_)));
    assert_eq!(sh(&dest, "git log -1 --format=%s"), "mine\n");
}

#[test]
fn a_branch_is_pushed_to_origin() {
    let tmp = tempfile::tempdir().unwrap();
    let url = remote_with_commit(tmp.path());
    let dest = tmp.path().join("clone");
    remote::clone(&url, &dest, TEST, None).unwrap();
    sh(
        &dest,
        "git config user.email a@a && git config user.name a && git checkout -qb agent/x && echo w > w.txt && git add . && git commit -qm work && git checkout -q main",
    );
    let pushed = remote::push(&dest, "agent/x", None).unwrap();
    assert_eq!(pushed.url, url);
    assert_eq!(sh(&tmp.path().join("remote.git"), "git log -1 --format=%s agent/x"), "work\n");
}
