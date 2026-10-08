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

#[repr(C)]
struct RLimit {
    cur: u64,
    max: u64,
}

unsafe extern "C" {
    fn getrlimit(resource: std::ffi::c_int, rlp: *mut RLimit) -> std::ffi::c_int;
    fn setrlimit(resource: std::ffi::c_int, rlp: *const RLimit) -> std::ffi::c_int;
}

/// Raises this process's open files limit (launchd starts it at 256: a socket per terminal, port
/// forward and VM connection runs out with a few VMs). Returns the limit now in force.
pub fn raise_open_files_limit() -> std::io::Result<u64> {
    const RLIMIT_NOFILE: std::ffi::c_int = 8;
    /// macOS refuses more than OPEN_MAX for this limit.
    const OPEN_MAX: u64 = 10240;
    let mut limit = RLimit { cur: 0, max: 0 };
    // SAFETY: valid pointers to a properly laid out struct for the duration of the calls.
    unsafe {
        if getrlimit(RLIMIT_NOFILE, &mut limit) != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let wanted = RLimit { cur: OPEN_MAX.min(limit.max).max(limit.cur), max: limit.max };
        if setrlimit(RLIMIT_NOFILE, &wanted) != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(wanted.cur)
    }
}
