//! The golden VM image: status and rebuild from the dashboard.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use serde::Serialize;

use crate::adapters::host::disk_usage;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct GoldenStatus {
    pub exists: bool,
    pub size_mb: u64,
    pub built_at: Option<f64>,
    pub claude_version: Option<String>,
    pub rebuilding: bool,
    pub last_result: Option<String>,
    pub log_tail: String,
}

#[derive(Default)]
struct RebuildState {
    running: bool,
    last_result: Option<String>,
}

pub struct GoldenService {
    home: PathBuf,
    script: PathBuf,
    state: Arc<Mutex<RebuildState>>,
    /// A build still running after this is stuck: it is stopped, with all it started.
    limit: Duration,
}

/// Downloading Debian, installing Claude Code, booting the VM twice: well under an hour.
const REBUILD_LIMIT: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, thiserror::Error)]
pub enum GoldenError {
    #[error("a rebuild is already running")]
    AlreadyRunning,
    #[error("could not start {0}: {1}")]
    Spawn(String, std::io::Error),
}

impl GoldenService {
    pub fn new(home: PathBuf, script: PathBuf) -> Self {
        GoldenService { home, script, state: Arc::default(), limit: REBUILD_LIMIT }
    }

    pub fn with_time_limit(self, limit: Duration) -> Self {
        GoldenService { limit, ..self }
    }

    fn disk(&self) -> PathBuf {
        self.home.join("golden/disk.raw")
    }
    fn log(&self) -> PathBuf {
        self.home.join("golden/rebuild.log")
    }

    pub fn status(&self) -> GoldenStatus {
        let meta = std::fs::metadata(self.disk()).ok();
        let setup_log = std::fs::read_to_string(self.home.join("golden/build/share/setup.log")).unwrap_or_default();
        let log = std::fs::read_to_string(self.log()).unwrap_or_default();
        let state = self.state.lock().unwrap();
        GoldenStatus {
            exists: meta.is_some(),
            size_mb: disk_usage(&self.disk()) >> 20,
            built_at: meta
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64()),
            claude_version: setup_log
                .lines()
                .find_map(|l| l.strip_suffix(" (Claude Code)").map(|v| v.trim().to_owned())),
            rebuilding: state.running,
            last_result: state.last_result.clone(),
            log_tail: log.lines().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n"),
        }
    }

    /// Runs `scripts/build-golden.sh` in the background; new VMs use the new image once it is done.
    pub fn rebuild(&self, claude_version: &str) -> Result<(), GoldenError> {
        let mut state = self.state.lock().unwrap();
        if state.running {
            return Err(GoldenError::AlreadyRunning);
        }
        let _ = std::fs::create_dir_all(self.home.join("golden"));
        let log =
            std::fs::File::create(self.log()).map_err(|e| GoldenError::Spawn(self.log().display().to_string(), e))?;
        let err = log.try_clone().map_err(|e| GoldenError::Spawn("log".into(), e))?;
        let mut child = tokio::process::Command::new(&self.script)
            .env("AGENTVM_HOME", &self.home)
            .env("AGENTVM_CLAUDE_VERSION", claude_version)
            .stdin(Stdio::null())
            .stdout(log)
            .stderr(err)
            // Its own process group: stopping a stuck build stops the VM it booted too.
            .process_group(0)
            .spawn()
            .map_err(|e| GoldenError::Spawn(self.script.display().to_string(), e))?;
        state.running = true;
        state.last_result = None;
        let shared = self.state.clone();
        let limit = self.limit;
        tokio::spawn(async move {
            let result = match tokio::time::timeout(limit, child.wait()).await {
                Ok(Ok(s)) if s.success() => "ok".to_owned(),
                Ok(Ok(s)) => format!("failed ({s})"),
                Ok(Err(e)) => format!("failed ({e})"),
                Err(_) => {
                    if let Some(pid) = child.id() {
                        kill_group(pid);
                    }
                    let _ = child.wait().await;
                    format!("failed: took longer than {} minutes", limit.as_secs().div_ceil(60))
                }
            };
            let mut state = shared.lock().unwrap();
            state.running = false;
            state.last_result = Some(result);
        });
        Ok(())
    }
}

/// SIGKILL to the whole process group `pid` leads.
fn kill_group(pid: u32) {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    // SAFETY: plain syscall; a negative pid names the process group.
    unsafe { kill(-(pid as i32), 9) };
}
