//! Repositories given as a link: cloned once into agentvm's folder, fetched before each launch,
//! and the VM's branch pushed back. A local bare repository stands in for GitHub.

use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

use agentvm::app::remote_repos::{open_with, push};
use agentvm::app::store::TaskRecord;
use agentvm::domain::git_remote::GitRemote;
use agentvm::domain::ids::{CommitSha, RepoPath, TaskId};

use crate::helpers::ctx;

const TEST: &[&str] = &["file"];

fn sh(dir: &Path, cmd: &str) -> String {
    let out = Command::new("sh").arg("-c").arg(cmd).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "{cmd}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A remote with one commit, as a link to it.
fn remote(root: &Path) -> GitRemote {
    std::fs::create_dir(root.join("src")).unwrap();
    sh(
        &root.join("src"),
        "git init -q -b main && git config user.email t@t && git config user.name t && echo one > a && git add . && git commit -qm one",
    );
    sh(root, "git clone -q --bare src remote.git");
    GitRemote {
        url: format!("file://{}", root.join("remote.git").display()),
        host: "github.com".into(),
        path: "acme/shop".into(),
    }
}

#[test]
fn a_link_is_cloned_once_then_brought_up_to_date() {
    let (home, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let ctx = ctx(home.path());
    let r = remote(root.path());
    let first = open_with(&ctx, &r, TEST).unwrap();
    assert!(first.cloned);
    assert_eq!(first.path, home.path().join("repos/github.com/acme/shop"));
    sh(&root.path().join("src"), "echo two > b && git add . && git commit -qm two && git push -q ../remote.git main");
    let again = open_with(&ctx, &r, TEST).unwrap();
    assert!(!again.cloned && again.note.is_none(), "{again:?}");
    assert_eq!(sh(&again.path, "git log -1 --format=%s"), "two\n");
}

/// "One VM per line" launches the same link several times at once: one clone, every launch served.
#[test]
fn launches_of_the_same_link_at_once_share_one_clone() {
    let (home, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let ctx = ctx(home.path());
    let r = remote(root.path());
    let opened: Vec<_> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..4).map(|_| s.spawn(|| open_with(&ctx, &r, TEST))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert!(opened.iter().all(|o| o.is_ok()), "{opened:?}");
    assert_eq!(opened.iter().filter(|o| o.as_ref().unwrap().cloned).count(), 1);
}

#[test]
fn a_vm_s_saved_branch_is_pushed_and_nothing_unsaved_is_explained() {
    let (home, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let ctx = ctx(home.path());
    let r = remote(root.path());
    let clone = open_with(&ctx, &r, TEST).unwrap().path;
    let sha = sh(&clone, "git rev-parse HEAD");
    let id = TaskId::generate(SystemTime::now(), &[9]);
    let rec = TaskRecord::new(
        id.clone(),
        RepoPath::new(clone.clone()).unwrap(),
        None,
        CommitSha::parse(sha.trim()).unwrap(),
        true,
    );
    ctx.store.insert(rec);

    let err = push(&ctx, &id).unwrap_err().to_string();
    assert!(err.contains("save"), "{err}");

    sh(
        &clone,
        &format!(
            "git config user.email a@a && git config user.name a && git checkout -qb {} && echo w > w && git add . && git commit -qm work && git checkout -q main",
            id.branch()
        ),
    );
    let pushed = push(&ctx, &id).unwrap();
    assert_eq!(pushed.branch, id.branch());
    assert_eq!(sh(&root.path().join("remote.git"), &format!("git log -1 --format=%s {}", id.branch())), "work\n");
}
