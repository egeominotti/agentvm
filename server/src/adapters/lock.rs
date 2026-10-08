//! Un solo server per AGENTVM_HOME: lock esclusivo su `<home>/server.lock`.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::path::Path;

unsafe extern "C" {
    fn flock(fd: std::ffi::c_int, operation: std::ffi::c_int) -> std::ffi::c_int;
}

const LOCK_EX: i32 = 2;
const LOCK_NB: i32 = 4;

/// Il lock dura quanto il valore (il kernel lo rilascia alla chiusura del file).
pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    pub fn acquire(home: &Path) -> io::Result<Self> {
        std::fs::create_dir_all(home)?;
        let file = OpenOptions::new().create(true).truncate(false).write(true).open(home.join("server.lock"))?;
        // SAFETY: descrittore valido di proprietà di `file`.
        if unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } == 0 {
            Ok(InstanceLock { _file: file })
        } else {
            Err(io::Error::last_os_error())
        }
    }
}
