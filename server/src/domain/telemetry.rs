//! A VM's telemetry over its whole life: samples averaged over 10 seconds, the CPU peak kept.

use serde::{Deserialize, Serialize};

/// One point of a VM's history. `at` is the host's clock (seconds since the epoch).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TelemetrySample {
    pub at: f64,
    pub cpu_pct: f32,
    /// The highest one-second CPU in the interval: averages hide short bursts.
    pub cpu_peak: f32,
    pub mem_used_mb: u64,
    /// What the balloon let the VM keep (`None` when unknown).
    pub mem_limit_mb: Option<u64>,
    pub disk_used_mb: u64,
    pub disk_read_bps: Option<u64>,
    pub disk_write_bps: Option<u64>,
    pub net_rx_bps: u64,
    pub net_tx_bps: u64,
    pub load1: f32,
}

impl TelemetrySample {
    /// The sample of one `VmMetrics` read at `at` (host clock), with the memory the VM may keep.
    pub fn from_metrics(m: &super::metrics::VmMetrics, at: f64, mem_limit_mb: u64) -> Self {
        TelemetrySample {
            at,
            cpu_pct: m.cpu_pct as f32,
            cpu_peak: m.cpu_pct as f32,
            mem_used_mb: m.mem_used_mb,
            mem_limit_mb: Some(mem_limit_mb),
            disk_used_mb: m.disk_used_mb,
            disk_read_bps: m.disk_read_bps,
            disk_write_bps: m.disk_write_bps,
            net_rx_bps: m.net_rx_bps,
            net_tx_bps: m.net_tx_bps,
            load1: m.load1 as f32,
        }
    }
}

/// Seconds averaged into one line.
pub const PERIOD_S: f64 = 10.0;
/// A longer gap between samples (the Mac asleep, a paused VM) is never averaged across.
const MAX_GAP_S: f64 = 60.0;

/// Turns one-second samples into one line every `PERIOD_S`.
#[derive(Debug, Default)]
pub struct Downsampler {
    bucket: Vec<TelemetrySample>,
}

impl Downsampler {
    /// The averaged line of the interval this sample closes, if any.
    pub fn push(&mut self, sample: TelemetrySample) -> Option<TelemetrySample> {
        let closes = match (self.bucket.first(), self.bucket.last()) {
            (Some(first), Some(last)) => {
                let jumped = sample.at < last.at || sample.at - last.at > MAX_GAP_S;
                jumped || sample.at - first.at >= PERIOD_S
            }
            _ => false,
        };
        let line = closes.then(|| average(&std::mem::take(&mut self.bucket)));
        self.bucket.push(sample);
        line
    }
}

fn average(samples: &[TelemetrySample]) -> TelemetrySample {
    let n = samples.len().max(1);
    let mean_u = |f: fn(&TelemetrySample) -> u64| samples.iter().map(f).sum::<u64>() / n as u64;
    let mean_opt = |f: fn(&TelemetrySample) -> Option<u64>| {
        let known: Vec<u64> = samples.iter().filter_map(f).collect();
        (!known.is_empty()).then(|| known.iter().sum::<u64>() / known.len() as u64)
    };
    let last = samples.last().cloned().unwrap_or_default();
    TelemetrySample {
        at: samples.first().map_or(0.0, |s| s.at),
        cpu_pct: samples.iter().map(|s| s.cpu_pct).sum::<f32>() / n as f32,
        cpu_peak: samples.iter().map(|s| s.cpu_peak.max(s.cpu_pct)).fold(0.0, f32::max),
        mem_used_mb: mean_u(|s| s.mem_used_mb),
        mem_limit_mb: last.mem_limit_mb,
        disk_used_mb: last.disk_used_mb,
        disk_read_bps: mean_opt(|s| s.disk_read_bps),
        disk_write_bps: mean_opt(|s| s.disk_write_bps),
        net_rx_bps: mean_u(|s| s.net_rx_bps),
        net_tx_bps: mean_u(|s| s.net_tx_bps),
        load1: samples.iter().map(|s| s.load1).sum::<f32>() / n as f32,
    }
}

/// At most `max` points for a chart: consecutive lines averaged together (peaks kept).
pub fn thin(points: &[TelemetrySample], max: usize) -> Vec<TelemetrySample> {
    if points.len() <= max || max == 0 {
        return points.to_vec();
    }
    points.chunks(points.len().div_ceil(max)).map(average).collect()
}
