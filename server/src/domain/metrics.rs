//! Telemetry sample written by `agentvm-metrics` inside the VM once per second.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListeningPort {
    pub port: u16,
    /// Process that owns the socket, when known.
    #[serde(default)]
    pub name: String,
}

/// A VM port reachable from the Mac on `127.0.0.1:host_port`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForwardedPort {
    pub port: u16,
    pub host_port: u16,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
