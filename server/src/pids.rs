//! What a pid is now, asked of the kernel: instant, no process spawned (a `ps` that is slow on a
//! busy Mac made running VMs look dead after a restart).

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> std::ffi::c_int;
    fn proc_pidpath(pid: i32, buffer: *mut std::ffi::c_void, size: u32) -> std::ffi::c_int;
}

/// What a pid is now (guards against reused pids).
#[derive(Debug, PartialEq, Eq)]
pub enum HelperPid {
    /// A running `agentvm-vm`.
    Ours,
    /// No process has this pid any more.
    Gone,
    /// Another program (the pid was reused).
    Other,
}

/// Asks the kernel for the program behind `pid`: instant, no process spawned, so a busy Mac
/// never makes a running VM look dead.
pub fn helper_pid(pid: u32) -> HelperPid {
    let Ok(pid) = i32::try_from(pid) else { return HelperPid::Gone };
    let mut buf = [0u8; 4096];
    // SAFETY: the buffer is valid for its whole length (PROC_PIDPATHINFO_MAXSIZE).
    let n = unsafe { proc_pidpath(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
    if n > 0 {
        let path = String::from_utf8_lossy(&buf[..n as usize]);
        return if path.ends_with("/agentvm-vm") || path == "agentvm-vm" { HelperPid::Ours } else { HelperPid::Other };
    }
    // No path: either the process is gone, or it is not ours to inspect (then not ours at all).
    // SAFETY: signal 0 only checks that the process exists.
    if unsafe { kill(pid, 0) } == 0 { HelperPid::Other } else { HelperPid::Gone }
}
