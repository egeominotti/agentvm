//! The VM helper binary, workspaces cloned from the golden image, and the root shell inside a VM.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use agentvm::adapters::jobdir::JobWorkspace;
use agentvm::adapters::vm::VmConfig;
use agentvm::domain::ids::TaskId;

pub(crate) fn helper() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bin/agentvm-vm")
}

pub(crate) fn golden() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap()).join("AgentVMs/golden/disk.raw")
}

/// Unique per test: tests run in parallel and sockets share one folder.
pub(crate) fn unique_id() -> TaskId {
    static NEXT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(1);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst).to_be_bytes();
    TaskId::generate(SystemTime::now(), &[n[0] ^ 0x5a, n[1]])
}

/// A test that panics must not leave its VM running: VMs are detached from their parent by design.
pub(crate) struct KillVms(pub(crate) PathBuf);
impl Drop for KillVms {
    fn drop(&mut self) {
        let _ = std::process::Command::new("pkill")
            .args(["-TERM", "-f", &format!("agentvm-vm --config {}", self.0.display())])
            .status();
    }
}

pub(crate) fn workspace(tmp: &tempfile::TempDir) -> JobWorkspace {
    let ws = JobWorkspace::create(tmp.path(), &unique_id()).unwrap();
    ws.clone_disk(&golden()).unwrap();
    ws
}

pub(crate) fn config(ws: &JobWorkspace) -> VmConfig {
    VmConfig {
        disk: ws.disk(),
        efivars: ws.efivars(),
        share: ws.share(),
        console: ws.console(),
        cpus: 2,
        memory_mb: 2048,
        seed_iso: None,
        pty_socket: None,
        balloon: None,
        direct: None,
    }
}

pub(crate) fn repo_with_bundle(ws: &JobWorkspace) -> String {
    let repo = tempfile::tempdir().unwrap().keep();
    let run = |args: &[&str]| {
        let out = std::process::Command::new("git").arg("-C").arg(&repo).args(args).output().unwrap();
        assert!(out.status.success(), "{args:?}");
        String::from_utf8(out.stdout).unwrap()
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "init"]);
    run(&["bundle", "create", "-q", ws.repo_bundle().to_str().unwrap(), "--all"]);
    run(&["rev-parse", "HEAD"]).trim().to_owned()
}

/// Runs `cmd` in the VM's root shell and returns what the terminal printed until it finished.
pub(crate) async fn shell(socket: &std::path::Path, cmd: &str) -> String {
    use agentvm::adapters::pty::{Frame, PtyConnection};
    let t0 = Instant::now();
    loop {
        // A marker unique to this call that only the output contains (tmux replays old screens).
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let marker = format!("__DONE_{n}__");
        if let Ok(mut pty) = PtyConnection::open(socket, "shell", 200, 50).await
            && pty.send(&Frame::Input(format!("{cmd}; echo __DONE\"_\"{n}__\n").into_bytes())).await.is_ok()
        {
            let mut seen = String::new();
            let deadline = Instant::now() + Duration::from_secs(40);
            while Instant::now() < deadline && !seen.contains(&marker) {
                if let Ok(Ok(Some(bytes))) = tokio::time::timeout(Duration::from_secs(1), pty.recv()).await {
                    seen.push_str(&String::from_utf8_lossy(&bytes));
                }
            }
            return seen;
        }
        assert!(t0.elapsed() < Duration::from_secs(30), "the shell never answered");
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}
