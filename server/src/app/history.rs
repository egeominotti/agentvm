//! Claude's history in a VM: its usage over time, kept next to the job, and the conversation
//! copied out of the guest (`share/claude/`), for as long as the job folder exists.

use std::path::PathBuf;

use crate::domain::usage::{AgentUsage, UsageSample};

/// `<job>/usage.jsonl`.
pub fn usage_file(ctx: &super::context::AppCtx, id: &crate::domain::ids::TaskId) -> PathBuf {
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
