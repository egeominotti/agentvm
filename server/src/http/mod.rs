//! HTTP interface: JSON API, SSE events and dashboard. Thin by design: a handler reads the
//! request, calls an app use case and shapes its answer; `error` maps app errors to statuses.
mod assets;
mod backups;
mod diagnostics;
mod dto;
mod error;
mod events;
mod golden;
mod guard;
mod proxy;
mod router;
mod session;
mod settings;
mod snapshots;
mod system;
mod tasks;
mod telemetry;
mod terminal;

pub use router::router;

/// What every handler gets: the application.
type Ctx = axum::extract::State<std::sync::Arc<crate::app::supervisor::AppCtx>>;
