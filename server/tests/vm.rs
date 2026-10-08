//! VM reali: richiedono `scripts/build.sh` e `scripts/build-golden.sh`.
//! Eseguire con `cargo test --test vm -- --ignored`.

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

fn workspace(tmp: &tempfile::TempDir) -> JobWorkspace {
    let ws = JobWorkspace::create(tmp.path(), &TaskId::generate(SystemTime::now(), [1, 2])).unwrap();
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
#[ignore = "richiede bin/agentvm-vm"]
async fn helper_reports_invalid_config() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = JobWorkspace::create(tmp.path(), &TaskId::generate(SystemTime::now(), [3, 4])).unwrap();
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &config(&ws)).unwrap();
    assert!(matches!(vm.next_event().await, Some(VmEvent::Error(m)) if m.contains("disco assente")));
    assert!(matches!(vm.wait().await, VmExit::Error(m) if m.contains("disco assente")));
}

#[tokio::test]
#[ignore = "richiede la golden"]
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
#[ignore = "richiede la golden"]
async fn terminate_stops_a_running_vm() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = workspace(&tmp);
    // Fermata subito dopo l'avvio, prima che il guest si spenga da solo.
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
#[ignore = "richiede la golden"]
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
    })
    .unwrap();
    ws.write_token(&Secret::new("sk-ant-oat01-not-a-real-token".into())).unwrap();
    let mut cfg = config(&ws);
    cfg.pty_socket = Some(ws.pty_socket());
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &cfg).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));

    // Il server PTY del guest può impiegare qualche istante a mettersi in ascolto.
    let t0 = Instant::now();
    let mut seen = String::new();
    'retry: while t0.elapsed() < Duration::from_secs(30) {
        let Ok(mut pty) = PtyConnection::open(&ws.pty_socket(), "shell", 100, 30).await else {
            tokio::time::sleep(Duration::from_millis(300)).await;
            continue;
        };
        pty.send(&Frame::Input(b"echo ciao-$((40+2))\n".to_vec())).await.unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(1), pty.recv()).await {
                Ok(Ok(Some(bytes))) => {
                    seen.push_str(&String::from_utf8_lossy(&bytes));
                    if seen.contains("ciao-42") {
                        break 'retry;
                    }
                }
                Ok(_) => continue 'retry,
                Err(_) => {}
            }
        }
    }
    assert!(seen.contains("ciao-42"), "output del terminale: {seen:?}");

    std::fs::write(ws.share().join("close.request"), "").unwrap();
    assert_eq!(tokio::time::timeout(Duration::from_secs(30), vm.wait()).await.unwrap(), VmExit::Clean);
    let r = ws.read_result().expect("result.json");
    assert_eq!(r.commits, 0);
}
