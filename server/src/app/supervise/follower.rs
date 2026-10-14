//! Follows `stream.jsonl` and publishes the agent's events while the VM is alive.

use std::path::PathBuf;
use std::sync::Arc;

use futures::StreamExt;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::adapters::tail::tail_records;
use crate::app::events::EventLog;
use crate::domain::agent_event::{AgentEvent, parse_line};
use crate::secret::Secret;

pub(super) struct Follower {
    stop: watch::Sender<bool>,
    handle: JoinHandle<()>,
}

impl Follower {
    /// Every line goes through `token.redact` before becoming a public event.
    pub(super) fn start(
        log: Arc<EventLog>,
        stream: PathBuf,
        token: Secret,
        on_result: impl Fn(f64, u64, u64) + Send + 'static,
    ) -> Self {
        let (stop, rx) = watch::channel(false);
        let handle = tokio::spawn(async move {
            let cursor = log.source_cursor().unwrap_or_default();
            let mut lines = std::pin::pin!(tail_records(stream, cursor, rx));
            while let Some(record) = lines.next().await {
                let events = parse_line(&token.redact(&record.line));
                for event in &events {
                    if let AgentEvent::Result { cost_usd, input_tokens, output_tokens, .. } = &event {
                        on_result(*cost_usd, *input_tokens, *output_tokens);
                    }
                }
                log.publish_agent_line(events, record.cursor);
            }
        });
        Follower { stop, handle }
    }

    /// Reads the last remaining lines and exits.
    pub(super) async fn finish(self) {
        self.stop.send_replace(true);
        let _ = self.handle.await;
    }
}
