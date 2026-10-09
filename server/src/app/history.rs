//! Claude's history in a VM: its usage over time, kept next to the job, and the conversation
//! copied out of the guest (`share/claude/`), for as long as the job folder exists.

use std::path::PathBuf;

use serde::Serialize;

use super::context::AppCtx;
use crate::domain::ids::TaskId;
use crate::domain::transcript::{HistoryEntry, parse_line};
use crate::domain::usage::{AgentUsage, UsageSample};

/// Raw session bytes read per page (entries are then clipped to 16 KB each).
const PAGE_BYTES: u64 = 4 << 20;

pub use crate::adapters::transcripts::Cursor;

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Conversation {
    pub entries: Vec<HistoryEntry>,
    /// Where to continue from (the same when there is nothing new).
    pub cursor: Cursor,
    /// More is already waiting: ask again at once.
    pub more: bool,
}

#[derive(Debug, thiserror::Error)]
#[error("task not found")]
pub struct NotFound;

/// What Claude's conversation in VM `id` gained since `cursor`, a page at a time.
pub fn conversation(ctx: &AppCtx, id: &TaskId, cursor: &Cursor) -> Result<Conversation, NotFound> {
    ctx.store.get(id).ok_or(NotFound)?;
    let dir = ctx.config.jobs().join(id.as_str()).join("share/claude");
    let (lines, cursor) = crate::adapters::transcripts::read_new(&dir, cursor, PAGE_BYTES);
    let read: u64 = lines.iter().map(|l| l.len() as u64 + 1).sum();
    Ok(Conversation {
        entries: lines.iter().flat_map(|l| parse_line(l)).collect(),
        cursor,
        more: read >= PAGE_BYTES / 2,
    })
}

/// Claude's usage over the VM's life.
pub fn usage(ctx: &AppCtx, id: &TaskId) -> Result<Vec<UsageSample>, NotFound> {
    ctx.store.get(id).ok_or(NotFound)?;
    Ok(crate::jsonl::read(&usage_file(ctx, id)))
}

/// `<job>/usage.jsonl`.
pub fn usage_file(ctx: &AppCtx, id: &TaskId) -> PathBuf {
    ctx.config.jobs().join(id.as_str()).join("usage.jsonl")
}

pub type SharedUsage = std::sync::Arc<std::sync::Mutex<UsageRecorder>>;

/// Appends a sample to `<job>/usage.jsonl` each time the usage changes.
pub struct UsageRecorder {
    file: PathBuf,
    last: Option<UsageSample>,
}

impl UsageRecorder {
    /// Picks up from the file's last sample: a restart adds no duplicate.
    pub fn new(file: PathBuf) -> Self {
        let last = crate::jsonl::last::<UsageSample>(&file);
        UsageRecorder { file, last }
    }

    /// One recorder per VM, shared by whatever reports its usage (the status line, the agent's
    /// results): two would each keep their own last sample and write duplicates.
    pub fn shared(file: PathBuf) -> SharedUsage {
        std::sync::Arc::new(std::sync::Mutex::new(Self::new(file)))
    }

    pub fn record(&mut self, usage: &AgentUsage) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
        let sample = UsageSample::of(usage, now);
        if self.last.as_ref().is_some_and(|l| l.same_as(&sample)) {
            return;
        }
        if let Err(e) = crate::jsonl::append(&self.file, &sample) {
            tracing::warn!(file = %self.file.display(), error = %e, "usage history not written");
            return;
        }
        self.last = Some(sample);
    }
}
