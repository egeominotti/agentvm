//! Adapter tests against real systems: git, APFS, processes, Keychain. No mocks.

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

    git.fetch_bundle(&tmp.path().join("out.bundle"), "agent/x").unwrap();
    assert!(git.rev_parse("agent/x").is_ok());
    assert_eq!(git.commit_count(&base, "agent/x").unwrap(), 1);
    assert!(git.diff(&base, "agent/x").unwrap().contains("new.txt"));
    // The local working tree was not touched.
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
    // A child spawned at that moment by another test may hold the descriptor until its exec.
    let t0 = std::time::Instant::now();
    while InstanceLock::acquire(tmp.path()).is_err() {
        assert!(t0.elapsed() < Duration::from_secs(2), "lock not released");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn secret_redacts_its_own_value() {
    let secret = Secret::new("sk-ant-oat01-abc".into());
    assert_eq!(secret.redact("token=sk-ant-oat01-abc fine"), "token=[REDACTED] fine");
    assert_eq!(secret.redact("nothing to hide"), "nothing to hide");
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

#[test]
fn settings_file_roundtrips_and_is_absent_at_first() {
    use agentvm::adapters::settings_file;
    use agentvm::domain::settings::{Model, Settings};
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    assert!(settings_file::load(&path).is_none());
    let s = Settings {
        max_vms: 6,
        cpus: 2,
        memory_mb: 2048,
        timeout_s: 900,
        model: Model::parse("opus").unwrap(),
        default_repo: Some("~/x".into()),
        claude_version: agentvm::domain::settings::ClaudeVersion::parse("2.1.290").unwrap(),
        s3: None,
    };
    settings_file::save(&path, &s).unwrap();
    assert_eq!(settings_file::load(&path).unwrap(), s);
}

#[test]
fn keychain_token_can_be_written_and_read_back() {
    let tmp = tempfile::tempdir().unwrap();
    let kc = TempKeychain(tmp.path().join("w.keychain-db"));
    sh(tmp.path(), &format!("security create-keychain -p x {}", kc.0.display()));
    let keychain = Keychain::new(Some(kc.0.clone()));
    keychain.write_token(&Secret::new("sk-ant-oat01-first_Token-1".into())).unwrap();
    keychain.write_token(&Secret::new("sk-ant-oat01-second_Token-2".into())).unwrap();
    assert_eq!(keychain.read_token().unwrap().expose(), "sk-ant-oat01-second_Token-2");
    assert!(keychain.write_token(&Secret::new("bad token\" ; rm -rf /".into())).is_err());
}

#[test]
fn host_info_reports_this_mac() {
    let host = agentvm::adapters::host::host_limits();
    assert!(host.cpus >= 1 && host.ram_mb >= 1024, "{host:?}");
}

#[test]
fn pty_socket_path_fits_the_unix_limit_even_for_a_deep_home() {
    let tmp = tempfile::tempdir().unwrap();
    let deep = tmp.path().join("a-very-long-folder-name-for-agentvm-home-that-goes-on-and-on/and/on/jobs");
    let id = task_id();
    let ws = JobWorkspace::create(&deep, &id).unwrap();
    let path = ws.pty_socket();
    assert!(path.as_os_str().len() < 104, "{} bytes: {}", path.as_os_str().len(), path.display());
    assert_eq!(path, JobWorkspace::pty_socket_of(&deep, &id));
    let dir = path.parent().unwrap();
    assert_eq!(std::fs::metadata(dir).unwrap().permissions().mode() & 0o777, 0o700);
}

#[test]
#[ignore = "needs the network"]
fn claude_releases_lists_real_versions() {
    let r = agentvm::adapters::releases::fetch().unwrap();
    let semver = |v: &str| {
        v.split('.').count() == 3 && v.split('.').all(|p| p.chars().next().is_some_and(|c| c.is_ascii_digit()))
    };
    assert!(semver(&r.latest) && semver(&r.stable), "{r:?}");
    assert!(r.versions.len() >= 10, "{r:?}");
    assert!(r.versions.contains(&r.latest), "{r:?}");
    assert!(semver(&r.versions[0]));
}

#[test]
fn snapshots_are_stored_listed_and_deleted() {
    use agentvm::adapters::snapshots::SnapshotStore;
    use agentvm::domain::snapshot::{SnapshotId, SnapshotMeta};
    let tmp = tempfile::tempdir().unwrap();
    let disk = tmp.path().join("disk.raw");
    let efi = tmp.path().join("efivars");
    std::fs::write(&disk, vec![3u8; 1 << 20]).unwrap();
    std::fs::write(&efi, b"efi").unwrap();
    let store = SnapshotStore::new(tmp.path().join("snapshots"));
    let id = SnapshotId::generate(std::time::SystemTime::now(), [1, 2]);
    let meta = SnapshotMeta {
        id: id.clone(),
        name: "before refactor".into(),
        source_task: "20261008-120000-abcd".into(),
        repo: "/tmp/repo".into(),
        base_sha: "a".repeat(40),
        model: "default".into(),
        claude_version: None,
        created_at: 1.0,
        size_mb: 0,
    };
    store.create(&meta, &disk, &efi).unwrap();
    let list = store.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "before refactor");
    assert_eq!(std::fs::read(store.disk(&id)).unwrap(), std::fs::read(&disk).unwrap());
    assert!(store.get(&id).is_some());
    store.delete(&id).unwrap();
    assert!(store.list().is_empty());
}

#[test]
fn snapshot_ids_reject_path_tricks() {
    use agentvm::domain::snapshot::SnapshotId;
    assert!(SnapshotId::parse("snap-20261008-120000-abcd").is_some());
    for bad in ["../etc", "snap-x", "20261008-120000-abcd", "snap-20261008-120000-abcd/.."] {
        assert!(SnapshotId::parse(bad).is_none(), "{bad}");
    }
}

#[test]
fn archives_pack_and_unpack_a_folder_exactly() {
    use agentvm::adapters::archive;
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir(&src).unwrap();
    std::fs::write(src.join("disk.raw"), vec![9u8; 3 << 20]).unwrap();
    std::fs::write(src.join("meta.json"), "{\"a\":1}").unwrap();
    let file = tmp.path().join("snap.tar.zst");
    archive::pack(&src, &file).unwrap();
    let out = tmp.path().join("out");
    archive::unpack(&file, &out).unwrap();
    assert_eq!(std::fs::read(out.join("disk.raw")).unwrap(), std::fs::read(src.join("disk.raw")).unwrap());
    assert_eq!(std::fs::read_to_string(out.join("meta.json")).unwrap(), "{\"a\":1}");
}

fn dev_s3() -> agentvm::adapters::s3::S3Client {
    use agentvm::domain::s3::S3Config;
    let cfg = S3Config {
        endpoint: "http://127.0.0.1:9100".into(),
        region: "us-east-1".into(),
        bucket: "agentvm-backups".into(),
        prefix: format!("test-{}", std::process::id()),
        access_key: "agentvm".into(),
        path_style: true,
    };
    agentvm::adapters::s3::S3Client::new(cfg, Secret::new("agentvm-local-secret".into()))
}

#[test]
#[ignore = "needs the dev S3 server: scripts/dev-s3.sh up"]
fn s3_client_round_trips_objects_on_a_real_server() {
    let s3 = dev_s3();
    s3.ensure_bucket().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let up = tmp.path().join("up.bin");
    std::fs::write(&up, vec![7u8; 2 << 20]).unwrap();
    assert_eq!(s3.put_file(&s3.config().key("a/b.bin"), &up).unwrap(), 1);
    s3.put_bytes(&s3.config().key("a/b.json"), b"{\"ok\":true}").unwrap();
    let listed = s3.list(&s3.config().key("a/")).unwrap();
    assert_eq!(listed.len(), 2, "{listed:?}");
    assert!(listed.iter().any(|o| o.key.ends_with("b.bin") && o.size == 2 << 20), "{listed:?}");
    let down = tmp.path().join("down.bin");
    s3.get_file(&s3.config().key("a/b.bin"), &down).unwrap();
    assert_eq!(std::fs::read(&down).unwrap(), std::fs::read(&up).unwrap());
    assert_eq!(s3.get_bytes(&s3.config().key("a/b.json")).unwrap(), b"{\"ok\":true}");
    s3.delete(&s3.config().key("a/b.bin")).unwrap();
    s3.delete(&s3.config().key("a/b.json")).unwrap();
    assert!(s3.list(&s3.config().key("a/")).unwrap().is_empty());
}

#[test]
#[ignore = "needs the dev S3 server: scripts/dev-s3.sh up"]
fn s3_client_reports_bad_credentials() {
    use agentvm::domain::s3::S3Config;
    let good = dev_s3();
    let bad =
        agentvm::adapters::s3::S3Client::new(S3Config { ..good.config().clone() }, Secret::new("wrong-secret".into()));
    assert!(bad.list("x/").is_err());
}

#[test]
#[ignore = "needs the dev S3 server: scripts/dev-s3.sh up"]
fn s3_large_files_go_up_in_parts() {
    let s3 = dev_s3().with_part_size(5 << 20);
    s3.ensure_bucket().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let up = tmp.path().join("big.bin");
    // 12 MiB of varied bytes: three parts (5 + 5 + 2).
    let data: Vec<u8> = (0..12u32 << 20).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
    std::fs::write(&up, &data).unwrap();
    let key = s3.config().key("multi/big.bin");
    let parts = s3.put_file(&key, &up).unwrap();
    assert_eq!(parts, 3);
    let down = tmp.path().join("down.bin");
    s3.get_file(&key, &down).unwrap();
    assert_eq!(std::fs::read(&down).unwrap(), data);
    s3.delete(&key).unwrap();
}
