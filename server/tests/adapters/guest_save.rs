//! The guest's save (guest/agentvm-save), run by bash on real repositories: the agent's commits
//! are never lost, whatever it did to its branch, and the repository's hooks cannot block it.

use std::path::Path;
use std::process::Command;

const SAVE_SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../guest/agentvm-save");

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// A repository on `main` with one commit (the base) and the agent's branch made from it.
struct Vm {
    _dir: tempfile::TempDir,
    work: std::path::PathBuf,
    job: std::path::PathBuf,
    base: String,
}

const BRANCH: &str = "agent/test";

fn vm() -> Vm {
    let dir = tempfile::tempdir().unwrap();
    let (work, job) = (dir.path().join("work"), dir.path().join("job"));
    std::fs::create_dir_all(&work).unwrap();
    std::fs::create_dir_all(&job).unwrap();
    git(&work, &["init", "-q", "-b", "main"]);
    git(&work, &["config", "user.name", "agentvm"]);
    git(&work, &["config", "user.email", "agentvm@localhost"]);
    std::fs::write(work.join("a.txt"), "a\n").unwrap();
    git(&work, &["add", "."]);
    git(&work, &["commit", "-qm", "base"]);
    let base = git(&work, &["rev-parse", "HEAD"]);
    git(&work, &["checkout", "-qb", BRANCH]);
    Vm { _dir: dir, work, job, base }
}

/// Runs the guest's save in `vm`: (commits, save error).
fn save(vm: &Vm) -> (u32, String) {
    let script = format!(
        r#"set -euo pipefail; log() {{ echo "$*" >> "$JOB/job.log"; }}; source "{SAVE_SCRIPT}"; set +e
JOB="{job}"; BRANCH="{BRANCH}"; BASE="{base}"; ID=test
cd "{work}" && save_work && printf '%s|%s' "$COMMITS" "$SAVE_ERROR""#,
        job = vm.job.display(),
        base = vm.base,
        work = vm.work.display(),
    );
    let out = Command::new("bash").arg("-c").arg(script).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (commits, error) =
        text.split_once('|').unwrap_or_else(|| panic!("{text} {}", String::from_utf8_lossy(&out.stderr)));
    (commits.parse().unwrap(), error.to_owned())
}

fn commit(vm: &Vm, file: &str) {
    std::fs::write(vm.work.join(file), file).unwrap();
    git(&vm.work, &["add", "."]);
    git(&vm.work, &["commit", "-qm", file]);
}

/// The agent commits, then checks out main to compare and stays there: its branch must keep
/// its commit (the save used to move the branch to main and report no changes).
#[test]
fn leaving_the_branch_for_main_keeps_the_agents_commits() {
    let vm = vm();
    commit(&vm, "work.txt");
    git(&vm.work, &["checkout", "-q", "main"]);
    let (commits, error) = save(&vm);
    assert_eq!((commits, error.as_str()), (1, ""));
    assert_eq!(git(&vm.work, &["log", "-1", "--format=%s", BRANCH]), "work.txt");
    assert!(vm.job.join("out.bundle").exists());
}

/// Work continued on a new branch made from the agent's: the agent's branch moves forward to it.
#[test]
fn a_branch_made_from_the_agents_brings_it_forward() {
    let vm = vm();
    commit(&vm, "one.txt");
    git(&vm.work, &["checkout", "-qb", "feature"]);
    commit(&vm, "two.txt");
    assert_eq!(save(&vm), (2, String::new()));
}

/// Changes not committed yet, made while on main, come back to the agent's branch.
#[test]
fn uncommitted_changes_on_another_branch_are_saved_on_the_agents() {
    let vm = vm();
    git(&vm.work, &["checkout", "-q", "main"]);
    std::fs::write(vm.work.join("new.txt"), "new").unwrap();
    assert_eq!(save(&vm), (1, String::new()));
    assert_eq!(git(&vm.work, &["show", &format!("{BRANCH}:new.txt")]), "new");
}

/// When the changes cannot go back to the agent's branch, the save fails (the VM and its disk
/// stay) instead of moving the branch and losing commits.
#[test]
fn changes_that_conflict_with_the_agents_branch_fail_the_save_and_lose_nothing() {
    let vm = vm();
    std::fs::write(vm.work.join("a.txt"), "agent\n").unwrap();
    git(&vm.work, &["commit", "-qam", "agent edit"]);
    git(&vm.work, &["checkout", "-q", "main"]);
    std::fs::write(vm.work.join("a.txt"), "main edit\n").unwrap();
    let (_, error) = save(&vm);
    assert!(error.contains("left agent/test"), "{error}");
    assert_eq!(git(&vm.work, &["log", "-1", "--format=%s", BRANCH]), "agent edit");
}

/// A hook that `--no-verify` does not skip (prepare-commit-msg waiting for a terminal, here
/// failing) cannot stop the save.
#[test]
fn the_repositorys_hooks_never_block_a_save() {
    let vm = vm();
    let hook = vm.work.join(".git/hooks/prepare-commit-msg");
    std::fs::write(&hook, "#!/bin/sh\nexec < /dev/tty\nexit 1\n").unwrap();
    Command::new("chmod").arg("755").arg(&hook).status().unwrap();
    std::fs::write(vm.work.join("x.txt"), "x").unwrap();
    assert_eq!(save(&vm), (1, String::new()));
}

/// A request is moved aside before it is read: one the host writes meanwhile is never deleted.
#[test]
fn a_newer_request_is_never_deleted_unanswered() {
    let vm = vm();
    std::fs::write(vm.job.join("close.request"), "first-id").unwrap();
    let script = format!(
        r#"source "{SAVE_SCRIPT}"; JOB="{job}"
id=$(take_request close.request); printf 'second-id' > "$JOB/close.request"; printf '%s' "$id""#,
        job = vm.job.display()
    );
    let out = Command::new("bash").arg("-c").arg(script).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "first-id");
    assert_eq!(std::fs::read_to_string(vm.job.join("close.request")).unwrap(), "second-id");
}
