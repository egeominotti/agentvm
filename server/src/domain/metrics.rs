//! Telemetry sample written by `agentvm-metrics` inside the VM once per second.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct VmMetrics {
    pub uptime_s: u64,
    pub cpus: u32,
    /// Busy share of all the VM's vCPUs, 0–100.
    pub cpu_pct: f64,
    pub load1: f64,
    pub mem_used_mb: u64,
    pub mem_total_mb: u64,
    pub disk_used_mb: u64,
    pub disk_total_mb: u64,
    pub net_rx_bps: u64,
    pub net_tx_bps: u64,
    pub procs: u32,
    /// Busiest processes; `cpu_pct` is per core (can exceed 100).
    pub top: Vec<ProcessSample>,
    /// TCP ports something listens on inside the VM.
    #[serde(default)]
    pub ports: Vec<ListeningPort>,
    /// Root disk reads and writes, bytes per second (`None`: a collector from before them).
    #[serde(default)]
    pub disk_read_bps: Option<u64>,
    #[serde(default)]
    pub disk_write_bps: Option<u64>,
    /// Processes using the most memory.
    #[serde(default)]
    pub top_mem: Vec<ProcessSample>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ListeningPort {
    pub port: u16,
    /// Process that owns the socket, when known.
    #[serde(default)]
    pub name: String,
}

/// Processes of the VM's own system whose ports are not services of the user's (Tailscale listens
/// on random ports for its peer API).
const SYSTEM_LISTENERS: &[&str] = &["tailscaled"];

/// The ports to open on the Mac: those of the user's services, not the system's.
pub fn user_services(listening: &[ListeningPort]) -> Vec<ListeningPort> {
    listening.iter().filter(|l| !SYSTEM_LISTENERS.contains(&l.name.as_str())).cloned().collect()
}

/// A VM port reachable from the Mac on `127.0.0.1:host_port`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ForwardedPort {
    pub port: u16,
    pub name: String,
    pub kind: PortKind,
    /// HTTP services: `http://<port>.<vm>.localhost:<server port>`, through the server's proxy.
    pub url: Option<String>,
    /// Other services: a direct TCP forward on `127.0.0.1:<host_port>`.
    pub host_port: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum PortKind {
    Http,
    Tcp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ProcessSample {
    pub name: String,
    pub cpu_pct: f64,
    pub mem_mb: u64,
}

impl VmMetrics {
    pub fn mem_pct(&self) -> f64 {
        if self.mem_total_mb == 0 { 0.0 } else { 100.0 * self.mem_used_mb as f64 / self.mem_total_mb as f64 }
    }
}

/// A running VM writes its numbers every second. Silent this long, it is not answering (frozen,
/// out of memory, its kernel stopped): it cannot save or close itself.
pub const UNRESPONSIVE_AFTER_S: f64 = 45.0;

/// How long a VM has been silent, when that is long enough to call it unresponsive. A VM that
/// never wrote numbers yet (still booting) is not judged.
pub fn unresponsive_for(metrics_at: Option<f64>, now: f64) -> Option<f64> {
    let silent = now - metrics_at?;
    (silent > UNRESPONSIVE_AFTER_S).then_some(silent)
}
