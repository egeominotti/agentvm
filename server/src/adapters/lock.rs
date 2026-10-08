//! One server per AGENTVM_HOME: exclusive lock on `<home>/server.lock`.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

unsafe extern "C" {
    fn flock(fd: std::ffi::c_int, operation: std::ffi::c_int) -> std::ffi::c_int;
}

const LOCK_EX: i32 = 2;
const LOCK_NB: i32 = 4;

/// The lock lives as long as the value (the kernel releases it when the file is closed).
pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    /// Also makes `home` private (0700): it holds repos, VM disks and the API token.
    pub fn acquire(home: &Path) -> io::Result<Self> {
        std::fs::create_dir_all(home)?;
        std::fs::set_permissions(home, std::fs::Permissions::from_mode(0o700))?;
        let file = OpenOptions::new().create(true).truncate(false).write(true).open(home.join("server.lock"))?;
        // SAFETY: valid descriptor owned by `file`.
        if unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } == 0 {
            Ok(InstanceLock { _file: file })
        } else {
            Err(io::Error::last_os_error())
        }
    }
}
