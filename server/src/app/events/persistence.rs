//! A source line and its normalized events share one journal record for restart consistency.
use super::{EventLog, HISTORY_MAX, OutputItem};
use crate::adapters::tail::Cursor;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(super) struct Batch {
    pub items: Vec<OutputItem>,
    pub last_seq: u64,
    pub source: Option<Cursor>,
}

impl EventLog {
    pub fn persistent(file: std::path::PathBuf) -> Self {
        let saved: Vec<serde_json::Value> = crate::jsonl::read(&file);
        let log = Self { file: Some(file), ..Self::new() };
        let mut history = log.history.lock().unwrap();
        let mut next = 1;
        for value in saved {
            if let Ok(batch) = serde_json::from_value::<Batch>(value.clone()) {
                next = next.max(batch.last_seq.saturating_add(1));
                if batch.source.is_some() {
                    *log.source.lock().unwrap() = batch.source;
                }
                for item in batch.items {
                    history.push_back((item.seq, item.item));
                    if history.len() > HISTORY_MAX {
                        history.pop_front();
                    }
                }
            } else if let Ok(item) = serde_json::from_value::<OutputItem>(value) {
                next = next.max(item.seq.saturating_add(1));
                history.push_back((item.seq, item.item));
                if history.len() > HISTORY_MAX {
                    history.pop_front();
                }
            }
        }
        log.next.store(next, std::sync::atomic::Ordering::Relaxed);
        let normalized = history.iter().any(|(_, item)| matches!(item, super::StreamItem::Agent(_)));
        log.import_pending.store(!normalized, std::sync::atomic::Ordering::Relaxed);
        drop(history);
        log
    }

    pub(super) fn persist_batch(&self, items: Vec<OutputItem>, replace: bool) {
        if let Some(file) = &self.file {
            let batch = Batch {
                items,
                last_seq: self.next.load(std::sync::atomic::Ordering::Relaxed).saturating_sub(1),
                source: self.source.lock().unwrap().clone(),
            };
            let result = if replace {
                compacted_bytes(batch).and_then(|bytes| crate::atomic_file::write(file, &bytes))
            } else {
                crate::jsonl::append(file, &batch)
            };
            if let Err(error) = result {
                tracing::warn!(%error,"task output could not be persisted");
            }
        }
    }
}

/// History entries remain separate records: a bounded suffix can omit old entries without
/// losing the final sequence or source checkpoint. The footer also covers an empty history.
fn compacted_bytes(batch: Batch) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for item in batch.items {
        let row = Batch { last_seq: item.seq, items: vec![item], source: None };
        serde_json::to_writer(&mut bytes, &row).map_err(std::io::Error::other)?;
        bytes.push(b'\n');
    }
    let footer = Batch { items: Vec::new(), last_seq: batch.last_seq, source: batch.source };
    serde_json::to_writer(&mut bytes, &footer).map_err(std::io::Error::other)?;
    bytes.push(b'\n');
    Ok(bytes)
}
