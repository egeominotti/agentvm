//! What a VM costs the Mac: disk flushes and memory.

use std::time::{Duration, Instant};

use agentvm::adapters::vm::{VmEvent, VmProcess};

use crate::helpers::{KillVms, config, helper, repo_with_bundle, shell, workspace};

/// apt, git and databases flush constantly: a guest flush must not become a full device flush
/// of the Mac's SSD (F_FULLFSYNC, ~3 ms each). 2000 synchronous 4 KB writes in well under 3 s.
#[tokio::test]
#[ignore = "requires the golden image"]
async fn guest_disk_flushes_are_cheap() {
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
    let t0 = Instant::now();
    while !shell(&sock, "echo ready-$((1+1))").await.contains("ready-2") {
        assert!(t0.elapsed() < Duration::from_secs(60), "the shell never became ready");
    }
    let out = shell(&sock, "s=$(date +%s%N); dd if=/dev/zero of=/root/flush.bin bs=4k count=2000 oflag=dsync 2>/dev/null; echo FLUSH_MS=$(( ($(date +%s%N) - s) / 1000000 ))").await;
    let ms: u64 = out
        .rsplit("FLUSH_MS=")
        .next()
        .and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("{out:?}"));
    eprintln!("2000 synchronous writes: {ms} ms");
    agentvm::adapters::jobdir::write_request(&ws.share(), "close.request").unwrap();
    let _ = tokio::time::timeout(Duration::from_secs(30), vm.wait()).await;
    assert!(ms < 3000, "2000 synchronous 4 KB writes took {ms} ms");
}

/// The balloon device deflates on OOM, so the guest's MemTotal stays put: what changes is the
/// memory available to it.
fn mem_available_mb(out: &str) -> Option<u64> {
    out.rsplit("MemAvailable:").next()?.split_whitespace().next()?.parse::<u64>().ok().map(|kb| kb / 1024)
}

/// The server sets how much memory a VM may keep; the helper inflates or deflates the balloon.
#[tokio::test]
#[ignore = "requires the golden image"]
async fn idle_memory_goes_back_to_the_mac_and_returns_on_demand() {
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
    cfg.memory_mb = 4096;
    cfg.pty_socket = Some(ws.pty_socket());
    cfg.balloon = Some(ws.balloon());
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &cfg, &ws.dir().join("vm.events")).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));
    let sock = ws.pty_socket();
    let t0 = Instant::now();
    while !shell(&sock, "echo ready-$((1+1))").await.contains("ready-2") {
        assert!(t0.elapsed() < Duration::from_secs(60), "the shell never became ready");
    }
    let total = || async { mem_available_mb(&shell(&sock, "grep MemAvailable /proc/meminfo").await) };
    assert!(total().await.unwrap() > 3000);

    ws.set_memory_target(1536).unwrap();
    let t0 = Instant::now();
    while total().await.is_none_or(|mb| mb > 1500) {
        assert!(t0.elapsed() < Duration::from_secs(30), "the balloon never inflated");
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    ws.set_memory_target(4096).unwrap();
    let t0 = Instant::now();
    while total().await.is_none_or(|mb| mb < 3000) {
        assert!(t0.elapsed() < Duration::from_secs(30), "the memory never came back");
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    agentvm::adapters::jobdir::write_request(&ws.share(), "close.request").unwrap();
    let _ = tokio::time::timeout(Duration::from_secs(30), vm.wait()).await;
}
