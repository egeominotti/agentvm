//! VM telemetry kept for the VM's whole life: one averaged sample every 10 seconds.

use agentvm::domain::metrics::VmMetrics;
use agentvm::domain::telemetry::{Downsampler, TelemetrySample};

fn sample(at: f64, cpu: f32) -> TelemetrySample {
    TelemetrySample { at, cpu_pct: cpu, cpu_peak: cpu, mem_used_mb: 1000, ..Default::default() }
}

#[test]
fn ten_seconds_become_one_averaged_sample_keeping_the_cpu_peak() {
    let mut d = Downsampler::default();
    for i in 0..10 {
        assert_eq!(d.push(sample(100.0 + f64::from(i), (i as f32 + 1.0) * 10.0)), None);
    }
    let line = d.push(sample(110.0, 5.0)).expect("a line after 10 s");
    assert_eq!(line.at, 100.0);
    assert!((line.cpu_pct - 55.0).abs() < 0.01, "{line:?}");
    assert_eq!(line.cpu_peak, 100.0);
    assert_eq!(line.mem_used_mb, 1000);
}

/// The Mac asleep for two hours: what came before is closed on its own, nothing is averaged
/// across the gap (and a clock going backwards does the same).
#[test]
fn a_jump_in_time_closes_the_bucket_without_averaging_across_it() {
    let mut d = Downsampler::default();
    for i in 0..5 {
        d.push(sample(f64::from(i), 50.0));
    }
    let closed = d.push(sample(7200.0, 90.0)).expect("closed by the jump");
    assert_eq!((closed.at, closed.cpu_pct), (0.0, 50.0));
    for i in 1..10 {
        assert_eq!(d.push(sample(7200.0 + f64::from(i), 90.0)), None);
    }
    assert_eq!(d.push(sample(7210.0, 90.0)).map(|l| l.at), Some(7200.0));
    let back = d.push(sample(7000.0, 10.0));
    assert!(back.is_some(), "a clock going backwards closes the bucket too");
}

/// A VM started before these measures existed still reports the others.
#[test]
fn metrics_from_an_older_collector_still_parse() {
    let old = serde_json::json!({
        "uptime_s": 60, "cpus": 4, "cpu_pct": 12.5, "load1": 0.3, "mem_used_mb": 700,
        "mem_total_mb": 4096, "disk_used_mb": 3000, "disk_total_mb": 20000, "net_rx_bps": 10,
        "net_tx_bps": 5, "procs": 90, "top": []
    });
    let m: VmMetrics = serde_json::from_value(old).unwrap();
    assert_eq!((m.disk_read_bps, m.disk_write_bps), (None, None));
    assert!(m.top_mem.is_empty());
}

/// A VM's whole life (days of 10 s lines) is drawn with at most 1000 points; bursts survive.
#[test]
fn a_long_history_is_thinned_keeping_its_peaks() {
    use agentvm::domain::telemetry::thin;
    let mut points: Vec<_> = (0..5000).map(|i| sample(f64::from(i) * 10.0, 10.0)).collect();
    points[2345].cpu_peak = 99.0;
    let thinned = thin(&points, 1000);
    assert!(thinned.len() <= 1000 && thinned.len() >= 900, "{}", thinned.len());
    assert_eq!(thinned[0].at, 0.0);
    assert!(thinned.iter().any(|p| p.cpu_peak == 99.0), "the burst was averaged away");
    assert_eq!(thin(&points[..10], 1000).len(), 10, "short histories stay as they are");
}
