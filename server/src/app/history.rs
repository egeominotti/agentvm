//! Claude's history in a VM: its usage over time, kept next to the job, and the conversation
//! copied out of the guest (`share/claude/`), for as long as the job folder exists.

use std::path::PathBuf;

use serde::Serialize;

use super::context::AppCtx;
use crate::domain::ids::TaskId;
use crate::domain::transcript::{HistoryEntry, parse_line};
use crate::domain::usage::{AgentUsage, UsageSample};

/// Session lines read per page.
const PAGE_LINES: usize = 500;

#[derive(Debug, Serialize)]
pub struct Conversation {
    pub entries: Vec<HistoryEntry>,
    /// The line to ask for next (the same when there is nothing new).
    pub next: usize,
}

#[derive(Debug, thiserror::Error)]
#[error("task not found")]
pub struct NotFound;

/// Claude's conversation in VM `id` from session line `after` on, a page at a time.
pub fn conversation(ctx: &AppCtx, id: &TaskId, after: usize) -> Result<Conversation, NotFound> {
    ctx.store.get(id).ok_or(NotFound)?;
    let dir = ctx.config.jobs().join(id.as_str()).join("share/claude");
    let page = crate::adapters::transcripts::read_page(&dir, after, PAGE_LINES);
    Ok(Conversation { entries: page.lines.iter().flat_map(|l| parse_line(l)).collect(), next: page.next })
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

/// Appends a sample to `<job>/usage.jsonl` each time the usage changes.
pub struct UsageRecorder {
    file: PathBuf,
    last: Option<UsageSample>,
}

impl UsageRecorder {
    /// Picks up from the file's last sample: a restart adds no duplicate.
    pub fn new(file: PathBuf) -> Self {
        let last = crate::jsonl::read::<UsageSample>(&file).pop();
        UsageRecorder { file, last }
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
