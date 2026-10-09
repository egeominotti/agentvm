//! Every URL the server answers (spec §5), each mapped to its resource's handler, all behind the
//! loopback guard.

use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::routing::{delete, get, post, put};

use super::{
    assets, backups, diagnostics, events, golden, guard, history, session, settings, snapshots, system, tasks,
    telemetry, terminal,
};
use crate::app::supervisor::AppCtx;

pub fn router(ctx: Arc<AppCtx>) -> Router {
    Router::new()
        .merge(dashboard())
        .merge(task_routes())
        .merge(snapshot_routes())
        .merge(host_routes())
        .layer(middleware::from_fn_with_state(ctx.clone(), guard::loopback_only))
        .with_state(ctx)
}

type Routes = Router<Arc<AppCtx>>;

fn dashboard() -> Routes {
    Router::new()
        .route("/", get(assets::dashboard))
        .route("/logo.svg", get(assets::logo))
        .route("/assets/{*path}", get(assets::asset))
        .route("/vendor/{*path}", get(assets::vendor))
        .route("/next", get(assets::next_redirect))
        .route("/next/", get(assets::next_index))
        .route("/next/{*path}", get(assets::next))
}

fn task_routes() -> Routes {
    Router::new()
        .route("/api/tasks", get(tasks::list).post(tasks::create))
        .route("/api/tasks/{id}", get(tasks::detail).delete(tasks::remove))
        .route("/api/tasks/{id}/events", get(events::events))
        .route("/api/tasks/{id}/diff", get(tasks::diff))
        .route("/api/tasks/{id}/diagnostics", get(diagnostics::diagnostics))
        .route("/api/tasks/{id}/telemetry", get(telemetry::telemetry))
        .route("/api/tasks/{id}/claude", get(history::claude))
        .route("/api/tasks/{id}/claude/usage", get(history::claude_usage))
        .route("/api/tasks/{id}/stop", post(tasks::stop))
        .route("/api/tasks/{id}/save", post(session::save))
        .route("/api/tasks/{id}/close", post(session::close))
        .route("/api/tasks/{id}/pty", get(terminal::pty))
        .route("/api/tasks/{id}/upload", post(session::upload).layer(DefaultBodyLimit::disable()))
        .route("/api/tasks/{id}/snapshot", post(snapshots::take))
        .route("/api/tasks/{id}/auto-snapshots", put(snapshots::set_auto))
}

fn snapshot_routes() -> Routes {
    Router::new()
        .route("/api/snapshots", get(snapshots::list))
        .route("/api/snapshots/{sid}", delete(snapshots::delete))
        .route("/api/snapshots/{sid}/restore", post(snapshots::restore))
        .route("/api/snapshots/{sid}/export", get(backups::export))
        .route("/api/snapshots/{sid}/backup", post(backups::backup))
        .route("/api/snapshots/import", post(backups::import).layer(DefaultBodyLimit::disable()))
        .route("/api/backups", get(backups::list))
        .route("/api/backups/{sid}", delete(backups::delete))
        .route("/api/backups/{sid}/restore", post(backups::restore))
}

fn host_routes() -> Routes {
    Router::new()
        .route("/api/status", get(system::status))
        .route("/api/storage", get(system::storage))
        .route("/api/storage/cleanup", post(system::cleanup))
        .route("/api/settings", get(settings::get).put(settings::put))
        .route("/api/settings/token", put(settings::put_token))
        .route("/api/settings/s3", get(backups::get_s3).put(backups::put_s3))
        .route("/api/settings/s3/test", post(backups::test_s3))
        .route("/api/golden", get(golden::status))
        .route("/api/golden/rebuild", post(golden::rebuild))
        .route("/api/claude/versions", get(golden::claude_versions))
}
