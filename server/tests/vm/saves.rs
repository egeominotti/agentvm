//! Saving the work inside a running VM.

use std::time::{Duration, Instant};

use agentvm::adapters::jobdir::JobWorkspace;
use agentvm::adapters::vm::{VmEvent, VmProcess};
use agentvm::domain::outcome::VmExit;

use crate::helpers::{KillVms, config, helper, repo_with_bundle, shell, workspace};

async fn save_reply(ws: &JobWorkspace) -> agentvm::domain::save::SaveReply {
    let done = ws.share().join("save.done");
    let _ = std::fs::remove_file(&done);
    agentvm::adapters::jobdir::write_request(&ws.share(), "save.request", "t1").unwrap();
    let t0 = Instant::now();
    loop {
        if let Some(r) = agentvm::guestfs::read(&done, 4096).and_then(|b| agentvm::domain::save::SaveReply::parse(&b)) {
            return r;
        }
        assert!(t0.elapsed() < Duration::from_secs(40), "no save.done");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Husky hooks, a git command holding index.lock: a save must never power off the VM.
#[tokio::test]
#[ignore = "requires the golden image"]
async fn saves_survive_git_failures_in_the_guest() {
    use agentvm::domain::save::SaveReply;
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
    let sock = ws.pty_socket();
    // Input sent before the guest's shell exists is lost: wait until it echoes.
    let t0 = Instant::now();
    while !shell(&sock, "echo ready-$((1+1))").await.contains("ready-2") {
        assert!(t0.elapsed() < Duration::from_secs(60), "the shell never became ready");
    }

    // A rejecting pre-commit hook, and a lock another git command releases two seconds later.
    shell(&sock, "cd /root/work && mkdir -p /tmp/h && printf '#!/bin/sh\\nexit 1\\n' > /tmp/h/pre-commit && chmod +x /tmp/h/pre-commit \
        && git config core.hooksPath /tmp/h && echo one > one.txt && touch .git/index.lock && { (sleep 2; rm -f .git/index.lock) & }").await;
    assert_eq!(save_reply(&ws).await, SaveReply::Saved { commits: 1 });

    // A lock that never goes away: the save fails, says why, and the VM keeps running.
    shell(&sock, "cd /root/work && echo two > two.txt && touch .git/index.lock").await;
    match save_reply(&ws).await {
        SaveReply::Failed(e) => assert!(e.contains("index.lock"), "{e}"),
        other => panic!("expected a failed save, got {other:?}"),
    }
    let alive = shell(&sock, "echo still-$((40+2))").await;
    assert!(
        alive.contains("still-42"),
        "the VM went away: {alive:?} {:?}",
        std::fs::read_to_string(ws.share().join("job.log"))
    );

    shell(&sock, "rm -f /root/work/.git/index.lock").await;
    agentvm::adapters::jobdir::write_request(&ws.share(), "close.request", "t1").unwrap();
    assert_eq!(tokio::time::timeout(Duration::from_secs(40), vm.wait()).await.unwrap(), VmExit::Clean);
    assert_eq!(ws.read_result().expect("result.json").commits, 2);
}
