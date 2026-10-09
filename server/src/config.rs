//! Configuration read once at startup and passed via constructors.

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
    /// Folder with `build-golden.sh`, for rebuilds started from the dashboard.
    pub scripts_dir: PathBuf,
    /// Free disk space below which launches, snapshots and imports are refused.
    pub min_free_mb: u64,
}

impl Config {
    pub fn from_env() -> Self {
        fn var<T: std::str::FromStr>(name: &str, default: T) -> T {
            std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
        }
        let memory_mb = var("AGENTVM_MEMORY_MB", 4096u64);
        let user_home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()));
        let default_helper = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("agentvm-vm")))
            .unwrap_or_else(|| PathBuf::from("agentvm-vm"));
        Config {
            home: std::env::var("AGENTVM_HOME").map(PathBuf::from).unwrap_or_else(|_| user_home.join("AgentVMs")),
            port: var("AGENTVM_PORT", 7777),
            concurrency: var("AGENTVM_CONCURRENCY", vms_fitting_in_ram(memory_mb)).max(1),
            cpus: var("AGENTVM_CPUS", 4),
            memory_mb,
            timeout_s: var("AGENTVM_TIMEOUT_S", 1800),
            vm_helper: std::env::var("AGENTVM_VM_HELPER").map(PathBuf::from).unwrap_or(default_helper),
            scripts_dir: std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().and_then(|bin| bin.parent()).map(|root| root.join("scripts")))
                .unwrap_or_else(|| PathBuf::from("scripts")),
            min_free_mb: var("AGENTVM_MIN_FREE_GB", 10u64) * 1024,
        }
    }

    pub fn golden(&self) -> PathBuf {
        self.home.join("golden/disk.raw")
    }

    /// The kernel built with the golden image, kept beside it (see `adapters::kernel`).
    pub fn golden_kernel(&self) -> PathBuf {
        self.home.join("golden/boot")
    }

    pub fn jobs(&self) -> PathBuf {
        self.home.join("jobs")
    }
}

/// Memory left to macOS and the user's apps.
const HOST_RESERVE_MB: u64 = 8 * 1024;

/// How many `memory_mb` VMs fit in the Mac's RAM without making it swap.
fn vms_fitting_in_ram(memory_mb: u64) -> usize {
    let ram_mb = std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u64>().ok())
        .map_or(16 * 1024, |bytes| bytes >> 20);
    (ram_mb.saturating_sub(HOST_RESERVE_MB) / memory_mb.max(512)) as usize
}
