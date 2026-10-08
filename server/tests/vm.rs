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
