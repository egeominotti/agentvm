//! Configurazione letta una volta all'avvio e passata per costruttore.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub home: PathBuf,
    pub port: u16,
    pub concurrency: usize,
    pub cpus: u32,
    pub memory_mb: u64,
    pub timeout_s: u64,
    pub vm_helper: PathBuf,
}

impl Config {
    pub fn from_env() -> Self {
        fn var<T: std::str::FromStr>(name: &str, default: T) -> T {
            std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
        }
        let user_home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()));
        let default_helper = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("agentvm-vm")))
            .unwrap_or_else(|| PathBuf::from("agentvm-vm"));
        Config {
            home: std::env::var("AGENTVM_HOME").map(PathBuf::from).unwrap_or_else(|_| user_home.join("AgentVMs")),
            port: var("AGENTVM_PORT", 7777),
            concurrency: var("AGENTVM_CONCURRENCY", 4usize).max(1),
            cpus: var("AGENTVM_CPUS", 4),
            memory_mb: var("AGENTVM_MEMORY_MB", 4096),
            timeout_s: var("AGENTVM_TIMEOUT_S", 1800),
            vm_helper: std::env::var("AGENTVM_VM_HELPER").map(PathBuf::from).unwrap_or(default_helper),
        }
    }

    pub fn golden(&self) -> PathBuf {
        self.home.join("golden/disk.raw")
    }

    pub fn jobs(&self) -> PathBuf {
        self.home.join("jobs")
    }
}
