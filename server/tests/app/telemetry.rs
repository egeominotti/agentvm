//! Telemetry the dashboard can trust: fresh or said to be stale, and the VM's whole history.

use agentvm::domain::metrics::VmMetrics;
use agentvm::domain::telemetry::TelemetrySample;

use crate::helpers::{ctx, git_repo, running_task};

fn metrics(uptime_s: u64) -> VmMetrics {
    serde_json::from_value(serde_json::json!({
        "uptime_s": uptime_s, "cpus": 4, "cpu_pct": 20.0, "load1": 0.5, "mem_used_mb": 900,
        "mem_total_mb": 4096, "disk_used_mb": 3000, "disk_total_mb": 20000, "net_rx_bps": 0,
        "net_tx_bps": 0, "procs": 80, "top": []
    }))
    .unwrap()
}

/// The same sample read again (the guest stopped writing) does not count as fresh.
#[test]
fn a_sample_that_does_not_change_is_not_fresh() {
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let id = running_task(home.path(), &repo, true);
    let ctx = ctx(home.path());
    ctx.store.insert(agentvm::app::store::Store::load(&home.path().join("jobs")).remove(0));
    ctx.store.record_metrics(&id, metrics(100));
    let first = ctx.store.get(&id).unwrap().metrics_at.expect("fresh");
    std::thread::sleep(std::time::Duration::from_millis(50));
    ctx.store.record_metrics(&id, metrics(100));
    assert_eq!(ctx.store.get(&id).unwrap().metrics_at, Some(first), "a repeated sample is not new");
    ctx.store.record_metrics(&id, metrics(101));
    assert!(ctx.store.get(&id).unwrap().metrics_at.unwrap() > first);
}

/// The history of a VM: the last hour, or its whole life thinned for a chart.
#[test]
fn the_history_is_served_per_range() {
    use agentvm::app::telemetry::{Range, series};
    let (home, repo) = (tempfile::tempdir().unwrap(), git_repo());
    let id = running_task(home.path(), &repo, true);
    let ctx = ctx(home.path());
    ctx.store.insert(agentvm::app::store::Store::load(&home.path().join("jobs")).remove(0));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
    let file = home.path().join("jobs").join(id.as_str()).join("telemetry.jsonl");
    for i in 0..3000 {
        let s = TelemetrySample { at: now - 30_000.0 + f64::from(i) * 10.0, ..Default::default() };
        agentvm::jsonl::append(&file, &s).unwrap();
    }
    let hour = series(&ctx, &id, Range::Hour).unwrap();
    assert!(hour.iter().all(|s| s.at >= now - 3600.0) && hour.len() >= 300, "{}", hour.len());
    assert!(series(&ctx, &id, Range::All).unwrap().len() <= 1000);
}
