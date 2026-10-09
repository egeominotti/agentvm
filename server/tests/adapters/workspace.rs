//! The job folder: the VM's disk, its shared folder, the token and the guest's result.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use agentvm::adapters::jobdir::JobWorkspace;
use agentvm::secret::Secret;

use crate::helpers::task_id;

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

/// The Tailscale key reaches the guest like the Claude token: a private file it reads once. One
/// left unread goes with the job folder's other secrets.
#[test]
fn workspace_tailscale_key_is_private_and_removed_on_drop() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = JobWorkspace::create(tmp.path(), &task_id()).unwrap();
    let spec = agentvm::domain::tailscale::TailscaleSpec { hostname: "agentvm-x-4f94".into(), ssh: true, tags: vec![] };
    let key = Secret::new("tskey-auth-kTest-0123456789".into());
    JobWorkspace::offer_tailnet(&ws.share(), &spec, Some(&key)).unwrap();
    assert!(ws.share().join("tailscale-spec.json").is_file());
    let key = ws.share().join(".tailscale-key");
    assert_eq!(std::fs::metadata(&key).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(std::fs::read_to_string(&key).unwrap(), "tskey-auth-kTest-0123456789");
    drop(ws);
    assert!(!key.exists());
}

/// A VM that left the tailnet does not join again at its next boot, nor shows its old report.
#[test]
fn a_vm_that_left_the_tailnet_forgets_it() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = JobWorkspace::create(tmp.path(), &task_id()).unwrap();
    let spec = agentvm::domain::tailscale::TailscaleSpec { hostname: "agentvm-x-4f94".into(), ssh: true, tags: vec![] };
    JobWorkspace::offer_tailnet(&ws.share(), &spec, None).unwrap();
    std::fs::write(ws.share().join("tailscale.json"), r#"{"name":"x"}"#).unwrap();
    JobWorkspace::forget_tailnet(&ws.share());
    assert!(!ws.share().join("tailscale-spec.json").exists());
    assert_eq!(ws.read_tailnet(), None);
}

/// What the guest says of the tailnet: joined (its name and addresses) or why not. Anything else
/// (no file yet, a torn write) is nothing to show.
#[test]
fn workspace_reads_the_tailnet_the_guest_joined() {
    use agentvm::domain::tailscale::Tailnet;
    let tmp = tempfile::tempdir().unwrap();
    let ws = JobWorkspace::create(tmp.path(), &task_id()).unwrap();
    let file = ws.share().join("tailscale.json");
    assert_eq!(ws.read_tailnet(), None);
    std::fs::write(&file, r#"{"name":"agentvm-demo-4f94.tail1234.ts.net","ips":["100.64.0.5","fd7a:115c::5"]}"#)
        .unwrap();
    assert_eq!(
        ws.read_tailnet(),
        Some(Tailnet {
            name: Some("agentvm-demo-4f94.tail1234.ts.net".into()),
            ips: vec!["100.64.0.5".into(), "fd7a:115c::5".into()],
            error: None
        })
    );
    std::fs::write(&file, r#"{"error":"invalid key: unable to validate"}"#).unwrap();
    assert_eq!(ws.read_tailnet().and_then(|t| t.error).as_deref(), Some("invalid key: unable to validate"));
    std::fs::write(&file, "{torn").unwrap();
    assert_eq!(ws.read_tailnet(), None);
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

/// The guest runs the scripts of the server that started it, not the ones baked in its image.
#[test]
fn workspace_ships_the_guest_runtime_of_this_server() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = JobWorkspace::create(&tmp.path().join("jobs"), &task_id()).unwrap();
    let guest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../guest");
    for f in
        ["agentvm-job", "agentvm-pty", "agentvm-metrics", "agentvm-statusline", "agentvm-claude", "config/tmux.conf"]
    {
        let shipped = ws.share().join("runtime").join(Path::new(f).file_name().unwrap());
        assert_eq!(std::fs::read(&shipped).ok(), Some(std::fs::read(guest.join(f)).unwrap()), "{f}");
    }
}

/// A VM's disk must be a file of its own: booted through a link it would write into whatever the
/// link points to (the golden image every VM starts from, which then breaks them all).
#[test]
fn a_disk_that_is_a_link_is_not_the_vms_own() {
    let tmp = tempfile::tempdir().unwrap();
    let id = agentvm::domain::ids::TaskId::generate(std::time::SystemTime::now(), &[9, 9]);
    let ws = agentvm::adapters::jobdir::JobWorkspace::create(tmp.path(), &id).unwrap();
    let golden = tmp.path().join("golden.raw");
    std::fs::write(&golden, vec![0u8; 4096]).unwrap();
    std::os::unix::fs::symlink(&golden, ws.disk()).unwrap();
    assert!(ws.check_own_disk().is_err());
    std::fs::remove_file(ws.disk()).unwrap();
    ws.clone_disk(&golden).unwrap();
    assert!(ws.check_own_disk().is_ok());
}
