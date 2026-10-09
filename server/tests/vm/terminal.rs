//! The VM's terminal over vsock: it answers, reports telemetry and echoes keystrokes quickly.

use std::time::{Duration, Instant};

use agentvm::adapters::jobdir::JobWorkspace;
use agentvm::adapters::vm::{VmEvent, VmProcess};
use agentvm::domain::outcome::VmExit;

use crate::helpers::{KillVms, config, helper, repo_with_bundle, workspace};

#[tokio::test]
#[ignore = "requires the golden image"]
async fn interactive_vm_serves_a_shell_over_vsock_and_closes_on_request() {
    use agentvm::adapters::pty::{Frame, PtyConnection};
    use agentvm::domain::spec::TaskSpec;
    use agentvm::secret::Secret;

    let tmp = tempfile::tempdir().unwrap();
    let _vms = KillVms(tmp.path().to_path_buf());
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
        restore: false,
    })
    .unwrap();
    ws.write_token(&Secret::new("sk-ant-oat01-not-a-real-token".into())).unwrap();
    let mut cfg = config(&ws);
    cfg.pty_socket = Some(ws.pty_socket());
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &cfg, &ws.dir().join("vm.events")).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));

    // The guest PTY server may take a moment to start listening.
    let t0 = Instant::now();
    let mut seen = String::new();
    'retry: while t0.elapsed() < Duration::from_secs(30) {
        let Ok(mut pty) = PtyConnection::open(&ws.pty_socket(), "shell", 100, 30).await else {
            tokio::time::sleep(Duration::from_millis(300)).await;
            continue;
        };
        if pty.send(&Frame::Input(b"echo hello-$((40+2))\n".to_vec())).await.is_err() {
            tokio::time::sleep(Duration::from_millis(300)).await;
            continue;
        }
        // Typed the moment the session opens, in a VM just booted (tmux still starting): the keys
        // must arrive. A closed connection (the guest not listening yet) is retried; keys lost
        // on a live one are the bug, never retried away.
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(1), pty.recv()).await {
                Ok(Ok(Some(bytes))) => {
                    seen.push_str(&String::from_utf8_lossy(&bytes));
                    if seen.contains("hello-42") {
                        break 'retry;
                    }
                }
                Ok(_) if seen.is_empty() => continue 'retry,
                Ok(_) => break 'retry,
                Err(_) => {}
            }
        }
        break;
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
pub(crate) async fn open_ready_shell(ws: &JobWorkspace) -> agentvm::adapters::pty::PtyConnection {
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
    let _vms = KillVms(tmp.path().to_path_buf());
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
        restore: false,
    })
    .unwrap();
    ws.write_token(&Secret::new("sk-ant-oat01-not-a-real-token".into())).unwrap();
    let mut cfg = config(&ws);
    cfg.pty_socket = Some(ws.pty_socket());
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &cfg, &ws.dir().join("vm.events")).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));

    let mut pty = open_ready_shell(&ws).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let mut samples = Vec::new();
    for c in "abcdefghijklmnopqrstuvwxyzabcdefghijklmn".chars() {
        let sent = Instant::now();
        pty.send(&Frame::Input(c.to_string().into_bytes())).await.unwrap();
        // Until the key shows: zsh also redraws the line (highlighting, suggestions), in pieces.
        let mut seen = String::new();
        while !seen.contains(c) {
            let bytes = tokio::time::timeout(Duration::from_secs(2), pty.recv()).await.unwrap().unwrap().unwrap();
            seen.push_str(&String::from_utf8_lossy(&bytes));
        }
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
