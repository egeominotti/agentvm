//! Why a VM failed, what to do, when each step happened, and the evidence: everything the
//! dashboard's Diagnostics panel shows, read from the task, its job folder and the server log.

use serde::Serialize;

use super::context::AppCtx;
use crate::domain::diagnosis::hint;
use crate::domain::ids::TaskId;
use crate::domain::task::{StateAt, TaskState};
use crate::guestfs;

/// The logs worth reading, in the order a person reads them: (title, path in the job folder).
const LOGS: [(&str, &str); 5] = [
    ("Job (inside the VM)", "share/job.log"),
    ("setup.sh", "share/setup.log"),
    ("Claude's errors", "share/claude.err"),
    ("Boot console", "console.log"),
    ("VM process events", "vm.events"),
];
/// Lines kept from the end of each log, and the most read of each file.
const TAIL_LINES: usize = 200;
const TAIL_BYTES: u64 = 64 * 1024;

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Diagnostics {
    /// The failure reason, or the state the VM is in.
    pub summary: String,
    pub hint: Option<String>,
    pub timeline: Vec<StateAt>,
    pub logs: Vec<LogTail>,
    /// This VM's lines of the server log, readable.
    pub server_log: Vec<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct LogTail {
    pub name: String,
    pub file: String,
    pub tail: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DiagnosticsError {
    #[error("task not found")]
    NotFound,
}

pub fn of(ctx: &AppCtx, id: &TaskId) -> Result<Diagnostics, DiagnosticsError> {
    let record = ctx.store.get(id).ok_or(DiagnosticsError::NotFound)?;
    let job = ctx.config.jobs().join(id.as_str());
    let logs: Vec<LogTail> = LOGS
        .iter()
        .filter_map(|(name, file)| {
            // Never through a symlink or a FIFO the guest planted, never the whole file.
            let bytes = guestfs::read_suffix(&job.join(file), TAIL_BYTES)?;
            // Logs are terminal output (the console above all): shown without control sequences.
            let text = crate::domain::console_text::readable(&String::from_utf8_lossy(&bytes));
            let lines: Vec<&str> = text.lines().collect();
            let tail = lines[lines.len().saturating_sub(TAIL_LINES)..].join("\n");
            Some(LogTail { name: (*name).to_owned(), file: (*file).to_owned(), tail })
        })
        .collect();
    let job_log = logs.iter().find(|l| l.file == "share/job.log").map_or("", |l| l.tail.as_str());
    let summary = match &record.state {
        TaskState::Failed { reason } => reason.clone(),
        other => other.kind().to_owned(),
    };
    let hint = matches!(record.state, TaskState::Failed { .. }).then(|| hint(&summary, job_log)).flatten();
    let server_log = crate::adapters::server_log::lines_for(&ctx.config.home.join("logs"), id.as_str(), TAIL_LINES)
        .iter()
        .map(|line| readable(line))
        .collect();
    Ok(Diagnostics { summary, hint, timeline: record.timeline, logs, server_log })
}

/// `2026-10-09T10:02:01.123Z WARN state changed from=preparing to=failed reason=…`
fn readable(json: &str) -> String {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return json.to_owned() };
    let mut out = format!(
        "{} {} {}",
        v["timestamp"].as_str().unwrap_or_default(),
        v["level"].as_str().unwrap_or_default(),
        v["fields"]["message"].as_str().unwrap_or_default()
    );
    if let Some(fields) = v["fields"].as_object() {
        for (k, val) in fields.iter().filter(|(k, _)| !matches!(k.as_str(), "message" | "task")) {
            let val = val.as_str().map_or_else(|| val.to_string(), str::to_owned);
            out.push_str(&format!(" {k}={val}"));
        }
    }
    out
}
