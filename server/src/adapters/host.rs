//! Facts about this Mac: cores, memory, disk usage of a folder.

use std::path::Path;
use std::process::Command;

use crate::domain::settings::HostLimits;

fn sysctl(name: &str) -> Option<u64> {
    let out = Command::new("sysctl").args(["-n", name]).output().ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

pub fn host_limits() -> HostLimits {
    HostLimits {
        cpus: sysctl("hw.ncpu").unwrap_or(4) as u32,
        ram_mb: sysctl("hw.memsize").map_or(16 * 1024, |b| b >> 20),
    }
}

/// Bytes actually allocated on disk (sparse VM disks count only what they use).
pub fn disk_usage(path: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    let Ok(meta) = std::fs::symlink_metadata(path) else { return 0 };
    if meta.is_dir() {
        std::fs::read_dir(path).map_or(0, |entries| entries.flatten().map(|e| disk_usage(&e.path())).sum())
    } else {
        meta.blocks() * 512
    }
}
