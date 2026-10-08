//! Processo `agentvm-vm`: una VM per processo, eventi JSON Lines su stdout.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader, Lines};
use tokio::process::{Child, ChildStdout, Command};

use crate::domain::outcome::VmExit;

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> std::ffi::c_int;
}

const SIGTERM: i32 = 15;

/// Protocollo §3.5 della spec.
#[derive(Debug, Clone, Serialize)]
pub struct VmConfig {
    pub disk: PathBuf,
    pub efivars: PathBuf,
    pub share: PathBuf,
    pub console: PathBuf,
    pub cpus: u32,
    pub memory_mb: u64,
    pub seed_iso: Option<PathBuf>,
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
}

#[derive(Debug, thiserror::Error)]
pub enum VmError {
    #[error("impossibile scrivere la configurazione della VM: {0}")]
    Config(std::io::Error),
    #[error("impossibile avviare {helper}: {source}")]
    Spawn { helper: String, source: std::io::Error },
}

pub struct VmProcess {
    child: Child,
    stdout: Lines<BufReader<ChildStdout>>,
    last_error: Option<String>,
}

impl VmProcess {
    pub fn spawn(helper: &Path, config_path: &Path, cfg: &VmConfig) -> Result<Self, VmError> {
        let json = serde_json::to_vec_pretty(cfg).expect("VmConfig serializzabile");
        std::fs::write(config_path, json).map_err(VmError::Config)?;
        let mut child = Command::new(helper)
            .arg("--config")
            .arg(config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|source| VmError::Spawn { helper: helper.display().to_string(), source })?;
        let stdout = BufReader::new(child.stdout.take().expect("stdout in pipe")).lines();
        Ok(VmProcess { child, stdout, last_error: None })
    }

    pub fn pid(&self) -> u32 {
        self.child.id().unwrap_or(0)
    }

    /// Prossimo evento; `None` quando il processo ha chiuso stdout.
    pub async fn next_event(&mut self) -> Option<VmEvent> {
        while let Ok(Some(line)) = self.stdout.next_line().await {
            let Ok(raw) = serde_json::from_str::<RawEvent>(&line) else { continue };
            let event = match raw.event.as_str() {
                "started" => VmEvent::Started,
                "stopped" => VmEvent::Stopped { seconds: raw.seconds },
                "error" => VmEvent::Error(raw.message),
                _ => continue,
            };
            if let VmEvent::Error(m) = &event {
                self.last_error = Some(m.clone());
            }
            return Some(event);
        }
        None
    }

    /// Chiede l'arresto (SIGTERM → stop forzato della VM, uscita 130).
    pub fn terminate(&self) {
        if let Some(pid) = self.child.id() {
            // SAFETY: PID del nostro processo figlio, ancora vivo.
            unsafe { kill(pid as i32, SIGTERM) };
        }
    }

    /// Consuma gli eventi rimanenti e attende la fine del processo.
    pub async fn wait(mut self) -> VmExit {
        while self.next_event().await.is_some() {}
        match self.child.wait().await.map(|s| s.code()) {
            Ok(Some(0)) => VmExit::Clean,
            Ok(Some(130)) => VmExit::Signaled,
            Ok(code) => VmExit::Error(self.last_error.unwrap_or_else(|| match code {
                Some(c) => format!("agentvm-vm è uscito con codice {c}"),
                None => "agentvm-vm terminato da un segnale".into(),
            })),
            Err(e) => VmExit::Error(format!("attesa di agentvm-vm fallita: {e}")),
        }
    }
}
