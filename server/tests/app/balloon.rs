//! Memory given back by idle terminals comes back as soon as they work, even across a restart.

use agentvm::adapters::jobdir::JobWorkspace;
use agentvm::app::balloon::Balloon;
use agentvm::domain::ids::TaskId;
use agentvm::domain::metrics::VmMetrics;

fn sample(cpu_pct: f64) -> VmMetrics {
    serde_json::from_value(serde_json::json!({
        "uptime_s": 600, "cpus": 4, "cpu_pct": cpu_pct, "load1": 1.0, "mem_used_mb": 700,
        "mem_total_mb": 4096, "disk_used_mb": 1, "disk_total_mb": 2, "net_rx_bps": 0,
        "net_tx_bps": 0, "procs": 90, "top": []
    }))
    .unwrap()
}

/// A server restarted while a terminal was idle and shrunk: the first busy sample gives the VM
/// all its memory back (it used to stay at "used + 1 GB" until idle again).
#[test]
fn a_shrunk_vm_found_after_a_restart_gets_its_memory_back_when_busy() {
    let jobs = tempfile::tempdir().unwrap();
    let ws = JobWorkspace::create(jobs.path(), &TaskId::generate(std::time::SystemTime::now(), [4, 2])).unwrap();
    ws.set_memory_target(1792).unwrap();
    // The old start ("it has all of it") never wrote anything: the VM stayed shrunk.
    Balloon::new(4096, None).adjust(&ws, &sample(80.0), false);
    assert_eq!(ws.memory_target(), Some(1792));
    let mut balloon = Balloon::new(4096, ws.memory_target());
    balloon.adjust(&ws, &sample(80.0), false);
    assert_eq!(ws.memory_target(), Some(4096));
}
