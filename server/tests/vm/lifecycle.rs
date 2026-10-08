//! Booting, stopping and reattaching to a VM.

use std::time::{Duration, Instant};

use agentvm::adapters::jobdir::JobWorkspace;
use agentvm::adapters::vm::{VmEvent, VmProcess};
use agentvm::domain::outcome::VmExit;

use crate::helpers::{KillVms, config, helper, unique_id, workspace};

#[tokio::test]
#[ignore = "requires bin/agentvm-vm"]
async fn helper_reports_invalid_config() {
    let tmp = tempfile::tempdir().unwrap();
    let _vms = KillVms(tmp.path().to_path_buf());
    let ws = JobWorkspace::create(tmp.path(), &unique_id()).unwrap();
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &config(&ws), &ws.dir().join("vm.events")).unwrap();
    assert!(matches!(vm.next_event().await, Some(VmEvent::Error(m)) if m.contains("disk missing")));
    assert!(matches!(vm.wait().await, VmExit::Error(m) if m.contains("disk missing")));
}

#[tokio::test]
#[ignore = "requires the golden image"]
async fn boots_golden_without_task_and_powers_off() {
    let tmp = tempfile::tempdir().unwrap();
    let _vms = KillVms(tmp.path().to_path_buf());
    let ws = workspace(&tmp);
    let t0 = Instant::now();
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &config(&ws), &ws.dir().join("vm.events")).unwrap();
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
    let _vms = KillVms(tmp.path().to_path_buf());
    let ws = workspace(&tmp);
    // Stopped right after boot, before the guest shuts down on its own.
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &config(&ws), &ws.dir().join("vm.events")).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));
    vm.terminate();
    assert_eq!(vm.wait().await, VmExit::Signaled);
}

#[tokio::test]
#[ignore = "requires the golden image"]
async fn helper_is_detached_from_the_spawning_process_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let _vms = KillVms(tmp.path().to_path_buf());
    let ws = workspace(&tmp);
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &config(&ws), &ws.dir().join("vm.events")).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));
    // Supervisors that stop the server kill its whole process tree: the VM must not be in it.
    let out = std::process::Command::new("ps").args(["-o", "ppid=", "-p", &vm.pid().to_string()]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1", "the helper is still our descendant");
    vm.terminate();
    assert_eq!(vm.wait().await, VmExit::Signaled);
}

#[tokio::test]
#[ignore = "needs the golden image"]
async fn a_vm_outlives_its_process_handle_and_can_be_reattached() {
    let tmp = tempfile::tempdir().unwrap();
    let _vms = KillVms(tmp.path().to_path_buf());
    let ws = workspace(&tmp);
    let events = ws.dir().join("vm.events");
    let mut vm = VmProcess::spawn(&helper(), &ws.config_path(), &config(&ws), &events).unwrap();
    assert_eq!(vm.next_event().await, Some(VmEvent::Started));
    let pid = vm.pid();
    drop(vm); // the server goes away: the VM must keep running
    tokio::time::sleep(Duration::from_millis(500)).await;
    let mut again = VmProcess::attach(pid, &events).expect("helper still alive");
    assert_eq!(again.next_event().await, Some(VmEvent::Started), "replays its own history");
    again.terminate();
    assert_eq!(again.wait().await, VmExit::Signaled);
}
