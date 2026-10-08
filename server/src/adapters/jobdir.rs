//! A job's directory: cloned disk, guest contract files, guaranteed cleanup.

use std::ffi::CString;
use std::fs;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::process::OutputWithin;

use crate::domain::ids::TaskId;
use crate::domain::outcome::GuestResult;
use crate::domain::spec::TaskSpec;
use crate::guestfs;
use crate::secret::Secret;

unsafe extern "C" {
    fn clonefile(src: *const std::ffi::c_char, dst: *const std::ffi::c_char, flags: u32) -> std::ffi::c_int;
    fn kill(pid: i32, sig: i32) -> std::ffi::c_int;
    fn getuid() -> u32;
}

/// Guest scripts of this build, installed by the image's `agentvm-boot` at every launch.
const RUNTIME: &[(&str, &str)] = &[
    ("agentvm-job", include_str!("../../../guest/agentvm-job")),
    ("agentvm-pty", include_str!("../../../guest/agentvm-pty")),
    ("agentvm-metrics", include_str!("../../../guest/agentvm-metrics")),
    ("agentvm-statusline", include_str!("../../../guest/agentvm-statusline")),
    ("agentvm-claude", include_str!("../../../guest/agentvm-claude")),
    ("tmux.conf", include_str!("../../../guest/config/tmux.conf")),
];

/// JSON the guest writes (metrics, usage, result) is a few KiB.
const SMALL_FILE: u64 = 1 << 20;
const JOB_LOG_MAX: u64 = 256 << 10;

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

    /// An existing job folder (e.g. after a server restart). Cleans up like `create` on drop.
    pub fn existing(jobs_root: &Path, id: &TaskId) -> Self {
        JobWorkspace { dir: jobs_root.join(id.as_str()), id: id.to_string() }
    }

    pub fn create(jobs_root: &Path, id: &TaskId) -> io::Result<Self> {
        let dir = jobs_root.join(id.as_str());
        let share = dir.join("share");
        fs::create_dir_all(&share)?;
        // The guest writes as different users (root, agent).
        fs::set_permissions(&share, fs::Permissions::from_mode(0o777))?;
        fs::create_dir_all(socket_dir())?;
        fs::set_permissions(socket_dir(), fs::Permissions::from_mode(0o700))?;
        let runtime = share.join("runtime");
        fs::create_dir_all(&runtime)?;
        for (name, text) in RUNTIME {
            fs::write(runtime.join(name), text)?;
        }
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
    /// Outside the shared folder: only the host decides how much memory the VM keeps.
    pub fn balloon(&self) -> PathBuf {
        self.dir.join("memory.target")
    }

    /// The memory the VM was last told it may keep, if it was told anything.
    pub fn memory_target(&self) -> Option<u64> {
        fs::read_to_string(self.balloon()).ok()?.trim().parse().ok()
    }

    pub fn set_memory_target(&self, mb: u64) -> io::Result<()> {
        let tmp = self.dir.join("memory.target.tmp");
        fs::write(&tmp, mb.to_string())?;
        fs::rename(tmp, self.balloon())
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
        serde_json::from_slice(&guestfs::read(&self.share().join("metrics.json"), SMALL_FILE)?).ok()
    }

    /// Cost and tokens copied by the Claude Code status line inside the VM.
    pub fn read_usage(&self) -> Option<crate::domain::usage::AgentUsage> {
        serde_json::from_slice(&guestfs::read(&self.share().join("usage.json"), SMALL_FILE)?).ok()
    }

    /// Lines written by the guest job (`[1.8s] repo ready on …`).
    pub fn job_log(&self) -> Vec<String> {
        guestfs::read_prefix(&self.share().join("job.log"), JOB_LOG_MAX)
            .map(|b| String::from_utf8_lossy(&b).lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    pub fn activity(&self) -> Option<String> {
        let b = guestfs::read(&self.share().join("activity"), 64)?;
        let s = String::from_utf8_lossy(&b);
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
        clone_file(golden, &self.disk())
    }

    /// EFI variables of a restored machine (the snapshot keeps its own boot entries).
    pub fn copy_efivars(&self, from: &Path) -> io::Result<()> {
        fs::copy(from, self.efivars()).map(drop)
    }

    /// Events written by the VM helper (kept after the VM stops).
    pub fn events(&self) -> PathBuf {
        self.dir.join("vm.events")
    }

    pub fn read_pid(&self) -> Option<u32> {
        fs::read_to_string(self.pid_path()).ok()?.trim().parse().ok()
    }

    pub fn write_pid(&self, pid: u32) -> io::Result<()> {
        fs::write(self.pid_path(), pid.to_string())
    }

    /// `None` if the file is missing or invalid: only a readable result counts for the outcome.
    pub fn read_result(&self) -> Option<GuestResult> {
        serde_json::from_slice(&guestfs::read(&self.share().join("result.json"), SMALL_FILE)?).ok()
    }

    pub fn has_out_bundle(&self) -> bool {
        guestfs::open_regular(&self.out_bundle()).is_some()
    }

    /// Last lines of the serial console, to diagnose a failed boot.
    pub fn console_tail(&self, lines: usize) -> String {
        // Only the end of the file: the guest decides how much it prints on its console.
        let text = guestfs::read_suffix(&self.console(), 64 * 1024).unwrap_or_default();
        let text = String::from_utf8_lossy(&text);
        let all: Vec<&str> = text.lines().collect();
        all[all.len().saturating_sub(lines)..].join("\n")
    }
}

impl Drop for JobWorkspace {
    fn drop(&mut self) {
        for p in [
            self.disk(),
            self.efivars(),
            self.share().join(".token"),
            self.repo_bundle(),
            self.pid_path(),
            self.pty_socket(),
        ] {
            let _ = fs::remove_file(p);
        }
    }
}

fn socket_dir() -> PathBuf {
    // SAFETY: getuid has no preconditions.
    PathBuf::from(format!("/tmp/agentvm-{}", unsafe { getuid() }))
}

/// At server startup: terminates VMs left over from a previous run and frees their disks.
pub fn cleanup_orphans(jobs_root: &Path, keep: &std::collections::HashSet<String>) {
    let Ok(entries) = fs::read_dir(jobs_root) else { return };
    for dir in entries
        .flatten()
        .map(|e| e.path())
        .filter(|d| !d.file_name().is_some_and(|n| keep.contains(&*n.to_string_lossy())))
    {
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
        .output_within(std::time::Duration::from_secs(5))
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().ends_with("agentvm-vm"))
        .unwrap_or(false)
}

/// Asks the guest for something by creating `<share>/<name>` holding the request's id (never
/// through a planted symlink).
pub fn write_request(share: &Path, name: &str, id: &str) -> io::Result<()> {
    guestfs::publish(&share.join(name), id.as_bytes())
}

/// Instant copy-on-write copy (APFS `clonefile(2)`); `dst` must not exist.
pub fn clone_file(src: &Path, dst: &Path) -> io::Result<()> {
    let src = CString::new(src.as_os_str().as_bytes())?;
    let dst = CString::new(dst.as_os_str().as_bytes())?;
    // SAFETY: C strings valid for the duration of the call.
    if unsafe { clonefile(src.as_ptr(), dst.as_ptr(), 0) } == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}
