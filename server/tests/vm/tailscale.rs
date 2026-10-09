//! Tailscale in a real VM, without a real tailnet: an auth key Tailscale refuses. The VM tries,
//! says why it could not join (never with the key in it), deletes the key, sends no logs, and its
//! terminal is up all along.

use std::time::{Duration, Instant};

use agentvm::adapters::jobdir::JobWorkspace;
use agentvm::adapters::vm::{VmEvent, VmProcess};
use agentvm::domain::spec::TaskSpec;
use agentvm::domain::tailscale::TailscaleSpec;
use agentvm::secret::Secret;

use crate::helpers::{KillVms, config, helper, repo_with_bundle, shell, workspace};

const BAD_KEY: &str = "tskey-auth-kNotReal1CNTRL-0123456789abcdefNotARealKey";

/// Waits for the guest's report on the tailnet.
async fn tailnet_report(ws: &JobWorkspace) -> agentvm::domain::tailscale::Tailnet {
    let t0 = Instant::now();
    loop {
        if let Some(t) = ws.read_tailnet() {
            return t;
        }
        assert!(t0.elapsed() < Duration::from_secs(120), "the VM never said how joining went");
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

#[tokio::test]
#[ignore = "requires the golden image and the network"]
async fn a_refused_key_is_reported_without_the_key_and_deleted() {
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
    let spec = TailscaleSpec { hostname: "agentvm-test-0000".into(), ssh: true, tags: vec![] };
    JobWorkspace::offer_tailnet(&ws.share(), &spec, Some(&Secret::new(BAD_KEY.into()))).unwrap();
    let mut cfg = config(&ws);
    cfg.pty_socket = Some(ws.pty_socket());
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &cfg, &ws.dir().join("vm.events")).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));

    let report = tailnet_report(&ws).await;
    let error = report.error.clone().unwrap_or_else(|| panic!("a refused key joined? {report:?}"));
    assert!(!error.contains(BAD_KEY) && !error.contains("NotARealKey"), "the key leaked: {error}");
    assert!(!ws.share().join(".tailscale-key").exists(), "the key stayed in the job folder");

    let sock = ws.pty_socket();
    let t0 = Instant::now();
    while !shell(&sock, "echo ready-$((1+1))").await.contains("ready-2") {
        assert!(t0.elapsed() < Duration::from_secs(60), "the terminal never came up");
    }
    let out = shell(
        &sock,
        "echo LOGS=$(grep -c '^TS_NO_LOGS_NO_SUPPORT=true' /etc/default/tailscaled)-KEY=$(test -e /run/agentvm/tailscale-key && echo here || echo gone)",
    )
    .await;
    assert!(out.contains("LOGS=1-KEY=gone"), "logging still on, or the key left in /run: {out:?}");

    agentvm::adapters::jobdir::write_request(&ws.share(), "close.request", "t1").unwrap();
    let _ = tokio::time::timeout(Duration::from_secs(30), vm.wait()).await;
}

/// Joining from a running VM's page: the VM booted without Tailscale, the host offers it and asks;
/// the guest starts joining at once, reports, and leaves when asked.
#[tokio::test]
#[ignore = "requires the golden image and the network"]
async fn a_running_vm_joins_and_leaves_when_asked() {
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
    let t0 = Instant::now();
    while !shell(&sock, "echo ready-$((1+1))").await.contains("ready-2") {
        assert!(t0.elapsed() < Duration::from_secs(60), "the terminal never came up");
    }
    assert_eq!(ws.read_tailnet(), None, "nothing asked, nothing tried");

    let spec = TailscaleSpec { hostname: "agentvm-test-0001".into(), ssh: true, tags: vec![] };
    JobWorkspace::offer_tailnet(&ws.share(), &spec, Some(&Secret::new(BAD_KEY.into()))).unwrap();
    agentvm::adapters::jobdir::write_request(&ws.share(), "tailscale-up.request", "j1").unwrap();
    let report = tailnet_report(&ws).await;
    assert!(report.error.is_some_and(|e| !e.contains("NotARealKey")), "{:?}", ws.read_tailnet());
    assert!(ws.share().join("tailscale-up.started").exists(), "the guest did not answer the request");

    agentvm::adapters::jobdir::write_request(&ws.share(), "tailscale-down.request", "j2").unwrap();
    let t0 = Instant::now();
    while !ws.share().join("tailscale-down.done").exists() {
        assert!(t0.elapsed() < Duration::from_secs(30), "the guest never left the tailnet");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let out = shell(&sock, "systemctl is-active tailscaled").await;
    assert!(out.contains("inactive") || out.contains("failed"), "tailscaled still running: {out:?}");

    agentvm::adapters::jobdir::write_request(&ws.share(), "close.request", "t1").unwrap();
    let _ = tokio::time::timeout(Duration::from_secs(30), vm.wait()).await;
}
