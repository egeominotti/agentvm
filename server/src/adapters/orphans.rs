//! VMs left over from a previous run of the server: stopped, and their disks freed.

use std::fs;
use std::path::{Path, PathBuf};

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> std::ffi::c_int;
}

/// At server startup: terminates VMs left over from a previous run and frees their disks. A VM
/// gets SIGTERM, then SIGKILL if it is still there after 10 s; its files go only once it is gone.
pub fn cleanup_orphans(jobs_root: &Path, keep: &std::collections::HashSet<String>) {
    let Ok(entries) = fs::read_dir(jobs_root) else { return };
    let orphans: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|d| !d.file_name().is_some_and(|n| keep.contains(&*n.to_string_lossy())))
        .collect();
    let helpers: Vec<i32> = orphans
        .iter()
        .filter_map(|dir| fs::read_to_string(dir.join("vm.pid")).ok()?.trim().parse::<u32>().ok())
        .filter(|&pid| crate::pids::helper_pid(pid) == crate::pids::HelperPid::Ours)
        .filter_map(|pid| i32::try_from(pid).ok())
        .collect();
    for &pid in &helpers {
        // SAFETY: signal to a pid the kernel says is our helper.
        unsafe { kill(pid, 15) };
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    // SAFETY: signal 0 only checks that the process exists.
    while helpers.iter().any(|&p| unsafe { kill(p, 0) } == 0) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    for &pid in &helpers {
        // SAFETY: as above; a helper that ignored SIGTERM must not keep its memory.
        unsafe { kill(pid, 9) };
    }
    for dir in orphans {
        for f in ["vm.pid", "disk.raw", "efivars", "share/.token", "share/repo.bundle"] {
            let _ = fs::remove_file(dir.join(f));
        }
    }
}
