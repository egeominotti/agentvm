//! The server's log: one JSON object per line in `<logs>/agentvm.log.<YYYY-MM-DD>` (a file a
//! day, 14 kept), and a readable line on stderr. Events about a VM carry its id as `task`.

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

/// Days of logs kept.
const KEEP_DAYS: usize = 14;

/// Sets up logging for the process; keep the guard alive (dropping it flushes the file).
/// `level` is an env-filter directive (`info`, `debug`, `agentvm=debug`…). `None` when logging
/// was already set up or the folder cannot be used: the server runs without a file log.
pub fn init(logs_dir: &Path, level: &str) -> Option<WorkerGuard> {
    std::fs::create_dir_all(logs_dir).ok()?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("agentvm.log")
        .max_log_files(KEEP_DAYS)
        .build(logs_dir)
        .ok()?;
    let (file, guard) = tracing_appender::non_blocking(appender);
    let filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().json().with_writer(file))
        .with(fmt::layer().compact().with_writer(std::io::stderr))
        .try_init()
        .ok()?;
    Some(guard)
}
