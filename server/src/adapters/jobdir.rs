//! A job's directory: cloned disk, guest contract files, guaranteed cleanup.

use std::ffi::CString;
use std::fs;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::domain::ids::TaskId;
use crate::domain::outcome::GuestResult;
use crate::domain::spec::TaskSpec;
use crate::secret::Secret;

unsafe extern "C" {
    fn clonefile(src: *const std::ffi::c_char, dst: *const std::ffi::c_char, flags: u32) -> std::ffi::c_int;
    fn kill(pid: i32, sig: i32) -> std::ffi::c_int;
    fn getuid() -> u32;
}

/// Owns `<jobs>/<id>/`. On `Drop` it deletes the disk, EFI variables, token and input bundle;
/// the logs (`stream.jsonl`, `result.json`, `job.log`, `console.log`) are kept.
pub struct JobWorkspace {
    dir: PathBuf,
    id: String,
}

impl JobWorkspace {
    /// Paths of an existing job, without taking ownership (no cleanup on drop).
    pub fn share_of(jobs_root: &Path, id: &TaskId) -> PathBuf {
        jobs_root.join(id.as_str()).join("share")
    }
    /// Unix socket paths are limited to 104 bytes on macOS, so sockets live in a short
    /// per-user folder instead of the (possibly deep) jobs root. Task ids are unique.
    pub fn disk_of(jobs_root: &Path, id: &TaskId) -> PathBuf {
        jobs_root.join(id.as_str()).join("disk.raw")
    }
    pub fn efivars_of(jobs_root: &Path, id: &TaskId) -> PathBuf {
        jobs_root.join(id.as_str()).join("efivars")
    }
    pub fn pty_socket_of(_jobs_root: &Path, id: &TaskId) -> PathBuf {
        socket_dir().join(format!("{id}.sock"))
    }

    /// Deletes a finished job's folder (logs included).
    pub fn remove_job(jobs_root: &Path, name: &str) -> io::Result<()> {
        fs::remove_dir_all(jobs_root.join(name))
    }

    pub fn create(jobs_root: &Path, id: &TaskId) -> io::Result<Self> {
        let dir = jobs_root.join(id.as_str());
        let share = dir.join("share");
        fs::create_dir_all(&share)?;
        // The guest writes as different users (root, agent).
        fs::set_permissions(&share, fs::Permissions::from_mode(0o777))?;
        fs::create_dir_all(socket_dir())?;
        fs::set_permissions(socket_dir(), fs::Permissions::from_mode(0o700))?;
        Ok(JobWorkspace { dir, id: id.to_string() })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
    pub fn share(&self) -> PathBuf {
        self.dir.join("share")
    }
    pub fn disk(&self) -> PathBuf {
        self.dir.join("disk.raw")
    }
    pub fn efivars(&self) -> PathBuf {
        self.dir.join("efivars")
    }
    pub fn console(&self) -> PathBuf {
        self.dir.join("console.log")
    }
    pub fn config_path(&self) -> PathBuf {
        self.dir.join("vm.json")
    }
    pub fn pid_path(&self) -> PathBuf {
        self.dir.join("vm.pid")
    }
    pub fn stream(&self) -> PathBuf {
        self.share().join("stream.jsonl")
    }
    pub fn repo_bundle(&self) -> PathBuf {
        self.share().join("repo.bundle")
    }
    pub fn pty_socket(&self) -> PathBuf {
        socket_dir().join(format!("{}.sock", self.id))
    }
    pub fn read_metrics(&self) -> Option<crate::domain::metrics::VmMetrics> {
        serde_json::from_slice(&fs::read(self.share().join("metrics.json")).ok()?).ok()
    }

    pub fn activity(&self) -> Option<String> {
        let s = fs::read_to_string(self.share().join("activity")).ok()?;
        let s = s.trim();
        (!s.is_empty()).then(|| s.to_owned())
    }
    pub fn out_bundle(&self) -> PathBuf {
        self.share().join("out.bundle")
    }

    pub fn write_spec(&self, spec: &TaskSpec) -> io::Result<()> {
        fs::write(self.share().join("task.json"), serde_json::to_vec_pretty(spec)?)
    }

    pub fn write_token(&self, token: &Secret) -> io::Result<()> {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(self.share().join(".token"))?;
        f.write_all(token.expose().as_bytes())
    }

    /// Instant copy-on-write copy (APFS `clonefile(2)`).
    pub fn clone_disk(&self, golden: &Path) -> io::Result<()> {
        let src = CString::new(golden.as_os_str().as_bytes())?;
        let dst = CString::new(self.disk().as_os_str().as_bytes())?;
        // SAFETY: C strings valid for the duration of the call.
        if unsafe { clonefile(src.as_ptr(), dst.as_ptr(), 0) } == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
    }

    /// EFI variables of a restored machine (the snapshot keeps its own boot entries).
    pub fn copy_efivars(&self, from: &Path) -> io::Result<()> {
        fs::copy(from, self.efivars()).map(drop)
    }

    pub fn write_pid(&self, pid: u32) -> io::Result<()> {
        fs::write(self.pid_path(), pid.to_string())
    }

    /// `None` if the file is missing or invalid: only a readable result counts for the outcome.
    pub fn read_result(&self) -> Option<GuestResult> {
        serde_json::from_slice(&fs::read(self.share().join("result.json")).ok()?).ok()
    }

    pub fn has_out_bundle(&self) -> bool {
        self.out_bundle().is_file()
    }

    /// Last lines of the serial console, to diagnose a failed boot.
    pub fn console_tail(&self, lines: usize) -> String {
        let text = fs::read_to_string(self.console()).unwrap_or_default();
        let all: Vec<&str> = text.lines().collect();
        all[all.len().saturating_sub(lines)..].join("\n")
    }
}

impl Drop for JobWorkspace {
    fn drop(&mut self) {
        for p in [self.disk(), self.efivars(), self.share().join(".token"), self.repo_bundle(), self.pid_path(), self.pty_socket()] {
            let _ = fs::remove_file(p);
        }
    }
}

fn socket_dir() -> PathBuf {
    // SAFETY: getuid has no preconditions.
    PathBuf::from(format!("/tmp/agentvm-{}", unsafe { getuid() }))
}

/// At server startup: terminates VMs left over from a previous run and frees their disks.
pub fn cleanup_orphans(jobs_root: &Path) {
    let Ok(entries) = fs::read_dir(jobs_root) else { return };
    for dir in entries.flatten().map(|e| e.path()) {
        if let Some(pid) = fs::read_to_string(dir.join("vm.pid")).ok().and_then(|s| s.trim().parse::<i32>().ok())
            && is_vm_helper(pid)
        {
            // SAFETY: signal to a PID verified to be our helper.
            unsafe { kill(pid, 15) };
        }
        for f in ["vm.pid", "disk.raw", "efivars", "share/.token", "share/repo.bundle"] {
            let _ = fs::remove_file(dir.join(f));
        }
    }
}

fn is_vm_helper(pid: i32) -> bool {
    Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().ends_with("agentvm-vm"))
        .unwrap_or(false)
}
