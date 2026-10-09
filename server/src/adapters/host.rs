//! Facts about this Mac: cores, memory, disk usage of a folder.

use std::ffi::{CString, c_char, c_int, c_void};
use std::path::Path;

use crate::domain::settings::HostLimits;

/// A number the kernel publishes (`sysctl -n <name>`), read with a system call: microseconds,
/// where running `sysctl` took milliseconds and a process.
fn sysctl(name: &str) -> Option<u64> {
    let name = CString::new(name).ok()?;
    let mut value = [0u8; 8];
    let mut len = value.len();
    // SAFETY: `value` has `len` writable bytes, and the kernel writes no more than `len`.
    let rc = unsafe { sysctlbyname(name.as_ptr(), value.as_mut_ptr().cast(), &mut len, std::ptr::null_mut(), 0) };
    match (rc, len) {
        (0, 4) => Some(u64::from(u32::from_ne_bytes([value[0], value[1], value[2], value[3]]))),
        (0, 8) => Some(u64::from_ne_bytes(value)),
        _ => None,
    }
}

pub fn host_limits() -> HostLimits {
    HostLimits {
        cpus: sysctl("hw.ncpu").unwrap_or(4) as u32,
        ram_mb: sysctl("hw.memsize").map_or(16 * 1024, |b| b >> 20),
    }
}

/// Memory this Mac has free for new work, as macOS judges it (`kern.memorystatus_level`, the
/// figure `memory_pressure` prints): free and reclaimable pages, not counting what is in use.
pub fn memory_free_mb() -> Option<u64> {
    let percent = sysctl("kern.memorystatus_level")?;
    let total_mb = sysctl("hw.memsize")? >> 20;
    (percent <= 100).then(|| total_mb * percent / 100)
}

/// Free space (MB) on the volume holding `path`, as `df` reports it (the same system call).
pub fn free_mb(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: an all-zero `StatFs` is valid (integers and byte arrays only).
    let mut fs: StatFs = unsafe { std::mem::zeroed() };
    // SAFETY: a NUL-terminated path and a properly laid out struct, for the duration of the call.
    if unsafe { statfs(path.as_ptr(), &mut fs) } != 0 {
        return None;
    }
    Some(fs.bavail.saturating_mul(u64::from(fs.bsize)) >> 20)
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

/// macOS's `struct statfs` (64-bit, as on arm64).
#[repr(C)]
struct StatFs {
    bsize: u32,
    iosize: i32,
    blocks: u64,
    bfree: u64,
    bavail: u64,
    files: u64,
    ffree: u64,
    fsid: [i32; 2],
    owner: u32,
    fs_type: u32,
    flags: u32,
    fssubtype: u32,
    fstypename: [c_char; 16],
    mntonname: [c_char; 1024],
    mntfromname: [c_char; 1024],
    flags_ext: u32,
    reserved: [u32; 7],
}

unsafe extern "C" {
    // On Intel Macs the 64-bit layout above has its own symbol; on Apple silicon it is the only one.
    #[cfg_attr(target_arch = "x86_64", link_name = "statfs$INODE64")]
    fn statfs(path: *const c_char, buf: *mut StatFs) -> c_int;
    fn sysctlbyname(
        name: *const c_char,
        old: *mut c_void,
        old_len: *mut usize,
        new: *mut c_void,
        new_len: usize,
    ) -> c_int;
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
