//! A VM's telemetry for the dashboard's charts: the live 5 minutes, the last hour, or its whole
//! life thinned to what a chart can draw.

use super::context::AppCtx;
use crate::adapters::telemetry_file;
use crate::domain::ids::TaskId;
use crate::domain::telemetry::{TelemetrySample, thin};

/// Points a chart of the VM's whole life gets at most.
const MAX_POINTS: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Range {
    FiveMinutes,
    Hour,
    All,
}

impl Range {
    pub fn parse(s: &str) -> Option<Range> {
        match s {
            "5m" => Some(Range::FiveMinutes),
            "1h" => Some(Range::Hour),
            "all" => Some(Range::All),
            _ => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("task not found")]
pub struct NotFound;

/// `<job>/telemetry.jsonl`: one line every 10 seconds of the VM's life.
pub fn history_file(ctx: &AppCtx, id: &TaskId) -> std::path::PathBuf {
    ctx.config.jobs().join(id.as_str()).join("telemetry.jsonl")
}

pub fn series(ctx: &AppCtx, id: &TaskId, range: Range) -> Result<Vec<TelemetrySample>, NotFound> {
    let record = ctx.store.get(id).ok_or(NotFound)?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
    Ok(match range {
        Range::FiveMinutes => record.live.into_iter().collect(),
        Range::Hour => telemetry_file::read(&history_file(ctx, id), Some(now - 3600.0)),
        Range::All => thin(&telemetry_file::read(&history_file(ctx, id), None), MAX_POINTS),
    })
}
