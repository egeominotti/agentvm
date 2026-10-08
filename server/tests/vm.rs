//! Real VMs: require `scripts/build.sh` and `scripts/build-golden.sh`.
//! Run with `cargo test --test vm -- --ignored`.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use agentvm::adapters::jobdir::JobWorkspace;
use agentvm::adapters::vm::{VmConfig, VmEvent, VmProcess};
use agentvm::domain::ids::TaskId;
use agentvm::domain::outcome::VmExit;

fn helper() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bin/agentvm-vm")
}

fn golden() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap()).join("AgentVMs/golden/disk.raw")
}

/// Unique per test: tests run in parallel and sockets share one folder.
fn unique_id() -> TaskId {
    static NEXT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(1);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst).to_be_bytes();
    TaskId::generate(SystemTime::now(), [n[0] ^ 0x5a, n[1]])
}

fn workspace(tmp: &tempfile::TempDir) -> JobWorkspace {
    let ws = JobWorkspace::create(tmp.path(), &unique_id()).unwrap();
    ws.clone_disk(&golden()).unwrap();
    ws
}

fn config(ws: &JobWorkspace) -> VmConfig {
    VmConfig {
        disk: ws.disk(),
        efivars: ws.efivars(),
        share: ws.share(),
        console: ws.console(),
        cpus: 2,
        memory_mb: 2048,
        seed_iso: None,
        pty_socket: None,
    }
}

#[tokio::test]
#[ignore = "requires bin/agentvm-vm"]
async fn helper_reports_invalid_config() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = JobWorkspace::create(tmp.path(), &unique_id()).unwrap();
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &config(&ws)).unwrap();
    assert!(matches!(vm.next_event().await, Some(VmEvent::Error(m)) if m.contains("disk missing")));
    assert!(matches!(vm.wait().await, VmExit::Error(m) if m.contains("disk missing")));
}

#[tokio::test]
#[ignore = "requires the golden image"]
async fn boots_golden_without_task_and_powers_off() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = workspace(&tmp);
    let t0 = Instant::now();
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &config(&ws)).unwrap();
    assert!(vm.pid() > 0);
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));
    assert_eq!(vm.wait().await, VmExit::Clean);
    assert!(t0.elapsed() < Duration::from_secs(20), "{:?}", t0.elapsed());
    let r = ws.read_result().expect("result.json");
    assert_eq!(r.error.as_deref(), Some("no_task"));
}

#[tokio::test]
#[ignore = "requires the golden image"]
async fn terminate_stops_a_running_vm() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = workspace(&tmp);
    // Stopped right after boot, before the guest shuts down on its own.
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &config(&ws)).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));
    vm.terminate();
    assert_eq!(vm.wait().await, VmExit::Signaled);
}

fn repo_with_bundle(ws: &JobWorkspace) -> String {
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

#[tokio::test]
#[ignore = "requires the golden image"]
async fn interactive_vm_serves_a_shell_over_vsock_and_closes_on_request() {
    use agentvm::adapters::pty::{Frame, PtyConnection};
    use agentvm::domain::spec::TaskSpec;
    use agentvm::secret::Secret;

    let tmp = tempfile::tempdir().unwrap();
    let ws = workspace(&tmp);
    let base = repo_with_bundle(&ws);
    ws.write_spec(&TaskSpec {
        id: "t".into(),
        prompt: String::new(),
        branch: "agent/t".into(),
        base_sha: base,
        timeout_s: 60,
        interactive: true,
        model: None,
        claude_version: None,
    })
    .unwrap();
    ws.write_token(&Secret::new("sk-ant-oat01-not-a-real-token".into())).unwrap();
    let mut cfg = config(&ws);
    cfg.pty_socket = Some(ws.pty_socket());
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &cfg).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));

    // The guest PTY server may take a moment to start listening.
    let t0 = Instant::now();
    let mut seen = String::new();
    'retry: while t0.elapsed() < Duration::from_secs(30) {
        let Ok(mut pty) = PtyConnection::open(&ws.pty_socket(), "shell", 100, 30).await else {
            tokio::time::sleep(Duration::from_millis(300)).await;
            continue;
        };
        pty.send(&Frame::Input(b"echo hello-$((40+2))\n".to_vec())).await.unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(1), pty.recv()).await {
                Ok(Ok(Some(bytes))) => {
                    seen.push_str(&String::from_utf8_lossy(&bytes));
                    if seen.contains("hello-42") {
                        break 'retry;
                    }
                }
                Ok(_) => continue 'retry,
                Err(_) => {}
            }
        }
    }
    assert!(seen.contains("hello-42"), "terminal output: {seen:?}");

    // Telemetry arrives once per second from inside the VM.
    let t1 = Instant::now();
    let metrics = loop {
        if let Some(m) = ws.read_metrics() {
            break m;
        }
        assert!(t1.elapsed() < Duration::from_secs(10), "no metrics.json from the guest");
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    assert_eq!(metrics.cpus, 2);
    assert!(metrics.mem_total_mb > 1500 && metrics.mem_used_mb > 0, "{metrics:?}");
    assert!(metrics.disk_total_mb > 10_000, "{metrics:?}");

    std::fs::write(ws.share().join("close.request"), "").unwrap();
    assert_eq!(tokio::time::timeout(Duration::from_secs(30), vm.wait()).await.unwrap(), VmExit::Clean);
    let r = ws.read_result().expect("result.json");
    assert_eq!(r.commits, 0);
}

/// Opens the shell once the guest PTY server is ready (before that, the bridge closes immediately).
async fn open_ready_shell(ws: &JobWorkspace) -> agentvm::adapters::pty::PtyConnection {
    use agentvm::adapters::pty::{Frame, PtyConnection};
    let t0 = Instant::now();
    'retry: loop {
        assert!(t0.elapsed() < Duration::from_secs(30), "shell never ready");
        let Ok(mut pty) = PtyConnection::open(&ws.pty_socket(), "shell", 100, 30).await else {
            tokio::time::sleep(Duration::from_millis(200)).await;
            continue;
        };
        pty.send(&Frame::Input(b"echo ready-$((1+1))\n".to_vec())).await.unwrap();
        let mut seen = String::new();
        while !seen.contains("ready-2") {
            match tokio::time::timeout(Duration::from_secs(5), pty.recv()).await {
                Ok(Ok(Some(bytes))) => seen.push_str(&String::from_utf8_lossy(&bytes)),
                _ => {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    continue 'retry;
                }
            }
        }
        return pty;
    }
}

/// The shell must respond to keystrokes like a local terminal.
#[tokio::test]
#[ignore = "requires the golden image"]
async fn shell_keystroke_echo_is_fast() {
    use agentvm::adapters::pty::Frame;
    use agentvm::domain::spec::TaskSpec;
    use agentvm::secret::Secret;

    let tmp = tempfile::tempdir().unwrap();
    let ws = workspace(&tmp);
    let base = repo_with_bundle(&ws);
    ws.write_spec(&TaskSpec { id: "t".into(), prompt: String::new(), branch: "agent/t".into(), base_sha: base, timeout_s: 60, interactive: true, model: None, claude_version: None }).unwrap();
    ws.write_token(&Secret::new("sk-ant-oat01-not-a-real-token".into())).unwrap();
    let mut cfg = config(&ws);
    cfg.pty_socket = Some(ws.pty_socket());
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &cfg).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));

    let mut pty = open_ready_shell(&ws).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let mut samples = Vec::new();
    for c in "abcdefghijklmnopqrstuvwxyzabcdefghijklmn".chars() {
        let sent = Instant::now();
        pty.send(&Frame::Input(c.to_string().into_bytes())).await.unwrap();
        let bytes = tokio::time::timeout(Duration::from_secs(2), pty.recv()).await.unwrap().unwrap().unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains(c), "unexpected echo: {bytes:?}");
        samples.push(sent.elapsed());
    }
    samples.sort();
    let median = samples[samples.len() / 2];
    let p95 = samples[samples.len() * 95 / 100];
    eprintln!("keystroke echo: median {median:?}, p95 {p95:?}");
    vm.terminate();
    vm.wait().await;
    assert!(median < Duration::from_millis(5), "median {median:?}");
    assert!(p95 < Duration::from_millis(15), "p95 {p95:?}");
}
