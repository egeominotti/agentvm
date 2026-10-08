//! Test degli adapter su sistemi reali: git, APFS, processi, Portachiavi. Nessun mock.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use agentvm::adapters::git::Git;
use agentvm::adapters::jobdir::JobWorkspace;
use agentvm::adapters::keychain::{Keychain, KeychainError};
use agentvm::adapters::tail::tail_lines;
use agentvm::domain::ids::{RepoPath, TaskId};
use agentvm::secret::Secret;
use futures::StreamExt;

fn sh(dir: &Path, cmd: &str) -> String {
    let out = Command::new("sh").arg("-c").arg(cmd).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "{cmd}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

fn new_repo(dir: &Path) {
    sh(dir, "git init -q -b main && git config user.email t@t && git config user.name t \
             && echo base > base.txt && git add . && git commit -qm base");
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

    // Il "guest": clona dal bundle, lavora su un branch e produce out.bundle.
    let work = tmp.path().join("work");
    sh(tmp.path(), &format!("git clone -q {} work", bundle.display()));
    sh(&work, &format!(
        "git config user.email a@a && git config user.name a && git checkout -q -b agent/x {} \
         && echo new > new.txt && git add . && git commit -qm new \
         && git bundle create -q ../out.bundle {}..agent/x", base.as_str(), base.as_str()));

    git.fetch_bundle(&tmp.path().join("out.bundle"), "agent/x").unwrap();
    assert!(git.rev_parse("agent/x").is_ok());
    assert_eq!(git.commit_count(&base, "agent/x").unwrap(), 1);
    assert!(git.diff(&base, "agent/x").unwrap().contains("new.txt"));
    // Il working tree locale non è stato toccato.
    assert_eq!(std::fs::read_to_string(repo_dir.join("dirty.txt")).unwrap(), "uncommitted");
    assert!(sh(&repo_dir, "git status --porcelain").contains("?? dirty.txt"));
    assert!(!repo_dir.join("new.txt").exists());
}

#[test]
fn git_rev_parse_unknown_ref_fails() {
    let tmp = tempfile::tempdir().unwrap();
    new_repo(tmp.path());
    let git = Git::new(RepoPath::new(tmp.path().to_path_buf()).unwrap());
    assert!(git.rev_parse("does-not-exist").is_err());
}

fn task_id() -> TaskId {
    TaskId::generate(std::time::SystemTime::now(), [0xab, 0xcd])
}

#[test]
fn workspace_clones_disk_and_cleans_up_on_drop() {
    let tmp = tempfile::tempdir().unwrap();
    let golden = tmp.path().join("golden.raw");
    std::fs::write(&golden, vec![7u8; 1 << 20]).unwrap();

    let ws = JobWorkspace::create(&tmp.path().join("jobs"), &task_id()).unwrap();
    assert_eq!(std::fs::metadata(ws.share()).unwrap().permissions().mode() & 0o777, 0o777);
    ws.clone_disk(&golden).unwrap();
    assert_eq!(std::fs::read(ws.disk()).unwrap(), std::fs::read(&golden).unwrap());
    std::fs::write(ws.share().join("stream.jsonl"), "{}\n").unwrap();
    std::fs::write(ws.share().join("repo.bundle"), "x").unwrap();
    let (disk, share) = (ws.disk(), ws.share());
    drop(ws);
    assert!(!disk.exists());
    assert!(!share.join("repo.bundle").exists());
    assert!(share.join("stream.jsonl").exists());
    assert!(golden.exists());
}

#[test]
fn workspace_token_is_private_and_secret_is_redacted() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = JobWorkspace::create(tmp.path(), &task_id()).unwrap();
    let secret = Secret::new("sk-ant-oat01-test".into());
    ws.write_token(&secret).unwrap();
    let token = ws.share().join(".token");
    assert_eq!(std::fs::metadata(&token).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(std::fs::read_to_string(&token).unwrap(), "sk-ant-oat01-test");
    assert_eq!(format!("{secret:?}"), "[REDACTED]");
    drop(ws);
    assert!(!token.exists());
}

#[test]
fn workspace_reads_guest_result() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = JobWorkspace::create(tmp.path(), &task_id()).unwrap();
    assert!(ws.read_result().is_none());
    std::fs::write(ws.share().join("result.json"), "{not json").unwrap();
    assert!(ws.read_result().is_none());
    std::fs::write(ws.share().join("result.json"), r#"{"status":"ok","claude_exit":0,"commits":2}"#).unwrap();
    assert_eq!(ws.read_result().unwrap().commits, 2);
    assert!(!ws.has_out_bundle());
}

#[tokio::test]
async fn tail_follows_a_growing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("stream.jsonl");
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let lines = tokio::spawn(tail_lines(path.clone(), stop_rx).collect::<Vec<String>>());

    let mut child = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(format!("for i in 1 2 3; do echo l$i >> {0}; sleep 0.3; done; printf tail >> {0}", path.display()))
        .spawn()
        .unwrap();
    child.wait().await.unwrap();
    stop_tx.send(true).unwrap();
    let got = tokio::time::timeout(Duration::from_secs(5), lines).await.unwrap().unwrap();
    assert_eq!(got, ["l1", "l2", "l3", "tail"]);
}

struct TempKeychain(std::path::PathBuf);
impl Drop for TempKeychain {
    fn drop(&mut self) {
        let _ = Command::new("security").arg("delete-keychain").arg(&self.0).output();
    }
}

#[test]
fn keychain_reads_token_from_a_real_keychain() {
    let tmp = tempfile::tempdir().unwrap();
    let kc = TempKeychain(tmp.path().join("t.keychain-db"));
    sh(tmp.path(), &format!("security create-keychain -p x {}", kc.0.display()));
    let keychain = Keychain::new(Some(kc.0.clone()));
    assert!(matches!(keychain.read_token(), Err(KeychainError::Missing)));
    sh(tmp.path(), &format!("security add-generic-password -s agentvm -a agentvm -w tok123 {}", kc.0.display()));
    assert_eq!(keychain.read_token().unwrap().expose(), "tok123");
}

#[test]
fn missing_token_message_tells_how_to_fix() {
    assert!(KeychainError::Missing.to_string().contains("security add-generic-password -s agentvm -a agentvm -w"));
}

#[test]
fn instance_lock_is_exclusive_until_dropped() {
    use agentvm::adapters::lock::InstanceLock;
    let tmp = tempfile::tempdir().unwrap();
    let first = InstanceLock::acquire(tmp.path()).unwrap();
    assert!(InstanceLock::acquire(tmp.path()).is_err());
    drop(first);
    // Un figlio creato in quel momento da un altro test può tenere il descrittore fino al suo exec.
    let t0 = std::time::Instant::now();
    while InstanceLock::acquire(tmp.path()).is_err() {
        assert!(t0.elapsed() < Duration::from_secs(2), "lock non rilasciato");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn secret_redacts_its_own_value() {
    let secret = Secret::new("sk-ant-oat01-abc".into());
    assert_eq!(secret.redact("token=sk-ant-oat01-abc fine"), "token=[REDACTED] fine");
    assert_eq!(secret.redact("niente da nascondere"), "niente da nascondere");
}

#[test]
fn pty_frames_are_type_length_payload() {
    use agentvm::adapters::pty::{Frame, encode};
    assert_eq!(encode(&Frame::Input(b"ls\n".to_vec())), [0, 0, 0, 0, 3, b'l', b's', b'\n']);
    let resize = encode(&Frame::Resize { cols: 120, rows: 40 });
    assert_eq!(&resize[..1], &[1]);
    let len = u32::from_be_bytes(resize[1..5].try_into().unwrap()) as usize;
    let body: serde_json::Value = serde_json::from_slice(&resize[5..5 + len]).unwrap();
    assert_eq!(body, serde_json::json!({"cols": 120, "rows": 40}));
}
