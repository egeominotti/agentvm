//! Several shells side by side in one VM: each its own, and an extra one closes for real.

use std::time::Duration;

use agentvm::adapters::pty::{Frame, PtyConnection};
use agentvm::adapters::vm::{VmEvent, VmProcess};
use agentvm::domain::spec::TaskSpec;
use agentvm::secret::Secret;

use crate::helpers::{KillVms, config, helper, repo_with_bundle, workspace};
use crate::terminal::open_ready_shell;

/// Runs `cmd` in session `session`; fails unless it prints `until` (the echoed command never does).
async fn run(socket: &std::path::Path, session: &str, cmd: &str, until: &str) -> String {
    let mut pty = PtyConnection::open(socket, session, 120, 30).await.unwrap();
    pty.send(&Frame::Input(format!("{cmd}\n").into_bytes())).await.unwrap();
    let mut seen = String::new();
    while !seen.contains(until) {
        match tokio::time::timeout(Duration::from_secs(8), pty.recv()).await {
            Ok(Ok(Some(bytes))) => seen.push_str(&String::from_utf8_lossy(&bytes)),
            other => panic!("{session}: no {until:?} in {seen:?} ({other:?})"),
        }
    }
    seen
}

#[tokio::test]
#[ignore = "requires the golden image"]
async fn extra_shells_are_separate_and_close_for_real() {
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
    let _ready = open_ready_shell(&ws).await;
    let socket = ws.pty_socket();

    run(&socket, "shell-2", "export MARK=two; echo set-$((1+1))", "set-2").await;
    run(&socket, "shell-3", "export MARK=three; echo set-$((1+2))", "set-3").await;
    // Each shell keeps its own state.
    run(&socket, "shell-2", "echo mark-$MARK-end", "mark-two-end").await;

    PtyConnection::kill(&socket, "shell-2").await.expect("closing shell-2");
    // Opened again, shell-2 is a new shell; shell-3 is untouched.
    run(&socket, "shell-2", "echo mark-${MARK:-none}-end", "mark-none-end").await;
    run(&socket, "shell-3", "echo mark-$MARK-end", "mark-three-end").await;
}
