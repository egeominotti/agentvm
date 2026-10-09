//! `agentvm-vm` process: one VM per process. The helper is detached from the server (own session,
//! launchd as parent) and appends its JSON Lines events to a file, so it outlives the server and a
//! new server can attach to it.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::domain::outcome::VmExit;

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> std::ffi::c_int;
    fn setsid() -> i32;
    fn fork() -> i32;
    fn write(fd: i32, buf: *const std::ffi::c_void, count: usize) -> isize;
    fn _exit(status: i32) -> !;
}

const SIGTERM: i32 = 15;

/// Protocol §3.5 of the spec.
#[derive(Debug, Clone, Serialize)]
pub struct VmConfig {
    pub disk: PathBuf,
    pub efivars: PathBuf,
    pub share: PathBuf,
    pub console: PathBuf,
    pub cpus: u32,
    pub memory_mb: u64,
    pub seed_iso: Option<PathBuf>,
    /// Unix socket on the Mac forwarded to the vsock port of the PTY server in the guest.
    pub pty_socket: Option<PathBuf>,
    /// File holding the memory (MB) the VM may keep; the helper follows it with the balloon.
    pub balloon: Option<PathBuf>,
    /// Booted straight into this kernel; without it, through EFI and GRUB from the disk.
    #[serde(flatten)]
    pub direct: Option<DirectBoot>,
}

/// A kernel, its initial ramdisk and its command line: the VM skips the firmware and GRUB.
#[derive(Debug, Clone, Serialize)]
pub struct DirectBoot {
    pub kernel: PathBuf,
    pub initrd: PathBuf,
    pub cmdline: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VmEvent {
    Started,
    Stopped { seconds: f64 },
    Error(String),
}

#[derive(Deserialize)]
struct RawEvent {
    event: String,
    #[serde(default)]
    seconds: f64,
    #[serde(default)]
    message: String,
    /// Exit code announced with `stopped`: 0 = the guest shut down, 130 = stopped by us.
    #[serde(default)]
    code: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum VmError {
    #[error("could not write the VM configuration: {0}")]
    Config(std::io::Error),
    #[error("could not start {helper}: {source}")]
    Spawn { helper: String, source: std::io::Error },
}

const SIGKILL: i32 = 9;
const POLL: Duration = Duration::from_millis(100);

pub struct VmProcess {
    pid: u32,
    events: PathBuf,
    offset: u64,
    pending: Vec<u8>,
    last_error: Option<String>,
    exit: Option<VmExit>,
}

impl VmProcess {
    /// Starts the helper detached (own session, events appended to `events`).
    pub fn spawn(helper: &Path, config_path: &Path, cfg: &VmConfig, events: &Path) -> Result<Self, VmError> {
        use std::io::Read;
        use std::os::fd::AsRawFd;
        let json = serde_json::to_vec_pretty(cfg).expect("VmConfig is serializable");
        std::fs::write(config_path, json).map_err(VmError::Config)?;
        let out = std::fs::OpenOptions::new().create(true).append(true).open(events).map_err(VmError::Config)?;
        let spawn_err = |source| VmError::Spawn { helper: helper.display().to_string(), source };
        let (mut pid_in, pid_out) = std::io::pipe().map_err(spawn_err)?;
        let pid_fd = pid_out.as_raw_fd();
        let mut cmd = Command::new(helper);
        cmd.arg("--config").arg(config_path).stdin(Stdio::null()).stdout(out).stderr(Stdio::null());
        // Double fork: the helper ends up in its own session with launchd as its parent, so neither
        // a signal to our process group nor a kill of our process tree reaches the VM.
        // SAFETY: only async-signal-safe calls (setsid, fork, write, _exit) between fork and exec.
        unsafe {
            std::os::unix::process::CommandExt::pre_exec(&mut cmd, move || {
                setsid();
                match fork() {
                    -1 => Err(std::io::Error::last_os_error()),
                    0 => Ok(()),
                    grandchild => {
                        let pid = grandchild.to_ne_bytes();
                        write(pid_fd, pid.as_ptr().cast(), pid.len());
                        _exit(0)
                    }
                }
            });
        }
        let mut intermediate = cmd.spawn().map_err(spawn_err)?;
        drop(cmd);
        drop(pid_out);
        let _ = intermediate.wait();
        let mut pid = [0u8; 4];
        pid_in.read_exact(&mut pid).map_err(spawn_err)?;
        Ok(Self::attach_unchecked(i32::from_ne_bytes(pid) as u32, events))
    }

    /// Re-attaches to a helper started by an earlier server, if it is still running.
    pub fn attach(pid: u32, events: &Path) -> Option<Self> {
        is_helper(pid).then(|| Self::attach_unchecked(pid, events))
    }

    fn attach_unchecked(pid: u32, events: &Path) -> Self {
        VmProcess { pid, events: events.to_path_buf(), offset: 0, pending: Vec::new(), last_error: None, exit: None }
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    fn alive(&self) -> bool {
        // SAFETY: signal 0 only checks that the process exists.
        unsafe { kill(self.pid as i32, 0) == 0 }
    }

    /// Reads whatever the helper appended since the last call.
    fn read_new(&mut self) {
        use std::io::{Read, Seek, SeekFrom};
        let Ok(mut f) = std::fs::File::open(&self.events) else { return };
        if f.seek(SeekFrom::Start(self.offset)).is_ok() {
            let mut buf = Vec::new();
            if let Ok(n) = f.read_to_end(&mut buf) {
                self.offset += n as u64;
                self.pending.extend_from_slice(&buf);
            }
        }
    }

    fn next_line(&mut self) -> Option<String> {
        let pos = self.pending.iter().position(|&b| b == b'\n')?;
        let line: Vec<u8> = self.pending.drain(..=pos).collect();
        Some(String::from_utf8_lossy(&line[..pos]).into_owned())
    }

    /// Next event, replaying the helper's history first; `None` once it has exited and every
    /// event has been read. Cancel-safe: state only changes between awaits.
    pub async fn next_event(&mut self) -> Option<VmEvent> {
        loop {
            while let Some(line) = self.next_line() {
                let Ok(raw) = serde_json::from_str::<RawEvent>(&line) else { continue };
                let event = match raw.event.as_str() {
                    "started" => VmEvent::Started,
                    "stopped" => {
                        self.exit = Some(if raw.code == 130 { VmExit::Signaled } else { VmExit::Clean });
                        VmEvent::Stopped { seconds: raw.seconds }
                    }
                    "error" => {
                        self.last_error = Some(raw.message.clone());
                        self.exit = Some(VmExit::Error(raw.message.clone()));
                        VmEvent::Error(raw.message)
                    }
                    _ => continue,
                };
                return Some(event);
            }
            let was_alive = self.alive();
            self.read_new();
            if self.pending.contains(&b'\n') {
                continue;
            }
            if !was_alive {
                return None;
            }
            tokio::time::sleep(POLL).await;
        }
    }

    /// Asks for a stop (SIGTERM → forced VM stop, `stopped` with code 130).
    pub fn terminate(&self) {
        // SAFETY: signal to our helper's pid.
        unsafe { kill(self.pid as i32, SIGTERM) };
    }

    /// Kills the helper (SIGKILL) if it ignores `terminate`.
    pub fn kill(&mut self) {
        // SAFETY: signal to our helper's pid.
        unsafe { kill(self.pid as i32, SIGKILL) };
    }

    /// Reads the remaining events and returns how the VM ended.
    pub async fn wait(&mut self) -> VmExit {
        while self.next_event().await.is_some() {}
        self.exit.clone().unwrap_or_else(|| {
            VmExit::Error(self.last_error.clone().unwrap_or_else(|| "agentvm-vm exited without a final event".into()))
        })
    }
}

/// The pid belongs to a running `agentvm-vm`.
pub fn is_helper(pid: u32) -> bool {
    crate::pids::helper_pid(pid) == crate::pids::HelperPid::Ours
}
