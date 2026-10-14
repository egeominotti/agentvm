//! Finite normalized agent output, including bounded import of legacy finished streams.
use super::context::AppCtx;
use super::events::OutputPage;
use crate::domain::ids::TaskId;

const LEGACY_BYTES: u64 = 8 << 20;

#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    #[error("task not found")]
    NotFound,
    #[error("limit must be between 1 and 500")]
    Limit,
}

pub fn page(ctx: &AppCtx, id: &TaskId, cursor: u64, limit: usize) -> Result<OutputPage, OutputError> {
    if !(1..=500).contains(&limit) {
        return Err(OutputError::Limit);
    }
    let record = ctx.store.get(id).ok_or(OutputError::NotFound)?;
    let log = ctx.store.log(id).ok_or(OutputError::NotFound)?;
    if record.state.is_terminal() && log.begin_legacy_import() {
        let path = ctx.config.jobs().join(id.as_str()).join("share/stream.jsonl");
        if let Some(mut file) = crate::guestfs::open_regular(&path) {
            use std::io::{Read, Seek, SeekFrom};
            let length = file.metadata().map_or(0, |meta| meta.len());
            let offset = length.saturating_sub(LEGACY_BYTES);
            let mut bytes = Vec::new();
            if file.seek(SeekFrom::Start(offset)).is_ok() && file.take(LEGACY_BYTES).read_to_end(&mut bytes).is_ok() {
                let start =
                    if offset > 0 { bytes.iter().position(|b| *b == b'\n').map_or(bytes.len(), |n| n + 1) } else { 0 };
                let token = super::agents::read_credential(ctx, &record.agent)
                    .unwrap_or_else(|_| crate::secret::Secret::new(String::new()));
                let events = bytes[start..]
                    .split(|b| *b == b'\n')
                    .filter(|line| !line.is_empty())
                    .flat_map(|line| {
                        crate::domain::agent_event::parse_line(&token.redact(&String::from_utf8_lossy(line)))
                    })
                    .collect();
                log.import_legacy(events, offset > 0);
            }
        }
    }
    Ok(log.page(cursor, limit))
}
