//! API routes (spec §5).

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::Request;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use futures::{SinkExt, Stream, StreamExt};

use super::dto::{ApiError, CreateTask, Created, PtyQuery, Saved, SettingsView, Status, TaskDto, TokenUpdate};
use crate::app::events::StreamItem;
use crate::app::golden::GoldenStatus;
use crate::app::queries::{DiffError, StorageUsage, cleanup_finished_jobs, storage_usage, task_diff};
use crate::app::session::{self, SessionError, TerminalInput};
use crate::app::store::TaskRecord;
use crate::app::supervisor::{AppCtx, NewTask, SubmitError, submit};
use crate::domain::ids::TaskId;
use crate::domain::settings::{ClaudeVersion, Settings};
use crate::secret::Secret;

type Ctx = State<Arc<AppCtx>>;

pub fn router(ctx: Arc<AppCtx>) -> Router {
    Router::new()
        .route("/", get(dashboard))
        .route("/api/status", get(status))
        .route("/api/tasks", get(list).post(create))
        .route("/api/tasks/{id}", get(detail).delete(remove_task))
        .route("/api/tasks/{id}/events", get(events))
        .route("/api/tasks/{id}/diff", get(diff))
        .route("/api/tasks/{id}/stop", post(stop))
        .route("/api/tasks/{id}/save", post(save))
        .route("/api/tasks/{id}/close", post(close))
        .route("/api/tasks/{id}/pty", get(pty))
        .route("/vendor/{file}", get(vendor))
        .route("/logo.svg", get(logo))
        .route("/assets/{file}", get(asset))
        .route("/api/settings", get(get_settings).put(put_settings))
        .route("/api/settings/token", put(put_token))
        .route("/api/golden", get(golden_status))
        .route("/api/claude/versions", get(claude_versions))
        .route("/api/tasks/{id}/snapshot", post(snapshot_task))
        .route("/api/snapshots", get(list_snapshots))
        .route("/api/snapshots/{sid}", axum::routing::delete(delete_snapshot))
        .route("/api/snapshots/{sid}/restore", post(restore_snapshot))
        .route("/api/snapshots/{sid}/export", get(export_snapshot))
        .route("/api/snapshots/{sid}/backup", post(backup_snapshot))
        .route("/api/snapshots/import", post(import_snapshot).layer(axum::extract::DefaultBodyLimit::disable()))
        .route("/api/tasks/{id}/upload", post(upload).layer(axum::extract::DefaultBodyLimit::disable()))
        .route("/api/backups", get(list_backups))
        .route("/api/backups/{sid}", axum::routing::delete(delete_backup))
        .route("/api/backups/{sid}/restore", post(restore_backup))
        .route("/api/settings/s3", get(get_s3).put(put_s3))
        .route("/api/settings/s3/test", post(test_s3))
        .route("/api/golden/rebuild", post(golden_rebuild))
        .route("/api/storage", get(storage))
        .route("/api/storage/cleanup", post(storage_cleanup))
        .layer(middleware::from_fn_with_state(ctx.clone(), loopback_only))
        .with_state(ctx)
}

/// Proxied VM services first; then defense against DNS rebinding and cross-origin requests (POST and WebSocket): loopback Host/Origin only.
async fn loopback_only(State(ctx): Ctx, req: Request, next: Next) -> Response {
    let port = ctx.config.port;
    // `<port>.<vm>.localhost`: a service inside a VM, never the dashboard or its API.
    let host = req.headers().get(header::HOST).and_then(|v| v.to_str().ok());
    if let Some((guest_port, vm)) = host.and_then(|h| crate::domain::hostname::parse_proxy_host(h, port)) {
        return super::proxy::forward(ctx.clone(), guest_port, vm, req).await;
    }
    if is_loopback_request(port, &req) {
        let mut res = next.run(req).await;
        // No other site may show the dashboard in a frame and trick clicks on it.
        let h = res.headers_mut();
        h.insert(header::X_FRAME_OPTIONS, header::HeaderValue::from_static("DENY"));
        h.insert(header::CONTENT_SECURITY_POLICY, header::HeaderValue::from_static("frame-ancestors 'none'"));
        res
    } else {
        ApiError(StatusCode::FORBIDDEN, "request not allowed: use http://127.0.0.1".into()).into_response()
    }
}

fn is_loopback_request(port: u16, req: &Request) -> bool {
    let allowed = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    let header = |name| req.headers().get(name).and_then(|v| v.to_str().ok());
    let host_ok = header(header::HOST).is_some_and(|h| allowed.iter().any(|a| a == h));
    let origin_ok = header(header::ORIGIN).is_none_or(|o| allowed.iter().any(|a| o == format!("http://{a}")));
    // Origin must be checked on GETs too: a cross-site WebSocket to a terminal is a GET.
    host_ok && origin_ok
}

async fn dashboard() -> Html<&'static str> {
    Html(include_str!("web/index.html"))
}

async fn status(State(ctx): Ctx) -> Json<Status> {
    let token = ctx.keychain.read_token();
    Json(Status {
        golden: ctx.config.golden().is_file(),
        token_hint: token.as_ref().err().map(ToString::to_string),
        token: token.is_ok(),
        concurrency: ctx.scheduler.concurrency(),
        running: ctx.store.running_count(),
        host: ctx.settings.limits(),
        ram_committed_mb: ctx.store.committed_memory_mb(),
    })
}

async fn create(State(ctx): Ctx, Json(req): Json<CreateTask>) -> Result<(StatusCode, Json<Created>), ApiError> {
    let claude_version = match req.claude_version.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => Some(ClaudeVersion::parse(v).map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?),
        None => None,
    };
    let new = NewTask {
        repo: &req.repo_path,
        prompt: req.prompt,
        base_ref: req.base_ref.as_deref(),
        interactive: req.interactive,
        model: req.model,
        claude_version,
        restore_from: None,
        cpus: req.cpus,
        memory_mb: req.memory_mb,
        label: None,
    };
    let id = submit(&ctx, new).map_err(|e| {
        let code = match e {
            SubmitError::NoGolden(_) | SubmitError::Token(_) => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::BAD_REQUEST,
        };
        ApiError(code, e.to_string())
    })?;
    Ok((StatusCode::CREATED, Json(Created { id: id.to_string() })))
}

async fn list(State(ctx): Ctx) -> Json<Vec<TaskDto>> {
    Json(ctx.store.list().into_iter().map(TaskDto::from).collect())
}

fn find(ctx: &AppCtx, id: &str) -> Result<(TaskId, TaskRecord), ApiError> {
    let id = TaskId::parse(id).ok_or_else(ApiError::not_found)?;
    let record = ctx.store.get(&id).ok_or_else(ApiError::not_found)?;
    Ok((id, record))
}

async fn detail(State(ctx): Ctx, Path(id): Path<String>) -> Result<Json<TaskDto>, ApiError> {
    Ok(Json(find(&ctx, &id)?.1.into()))
}

async fn events(
    State(ctx): Ctx,
    Path(id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    let log = ctx.store.log(&id).ok_or_else(ApiError::not_found)?;
    let stream = log.stream().map(|(seq, item)| {
        let event = Event::default().id(seq.to_string());
        Ok(match item {
            StreamItem::State(s) => event.event("state").json_data(s),
            StreamItem::Agent(a) => event.event("agent").json_data(a),
        }
        .expect("events are always serializable"))
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

async fn diff(State(ctx): Ctx, Path(id): Path<String>) -> Result<impl IntoResponse, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    let text = task_diff(&ctx, &id).await.map_err(|e| {
        let code = match e {
            DiffError::NotFound => StatusCode::NOT_FOUND,
            DiffError::NoBranch => StatusCode::CONFLICT,
            DiffError::Git(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError(code, e.to_string())
    })?;
    Ok(([("content-type", "text/plain; charset=utf-8")], text))
}

async fn stop(State(ctx): Ctx, Path(id): Path<String>) -> Result<Json<TaskDto>, ApiError> {
    let (id, record) = find(&ctx, &id)?;
    if record.state.is_terminal() {
        return Err(ApiError(StatusCode::CONFLICT, "the task has already finished".into()));
    }
    ctx.store.request_stop(&id).map_err(|e| ApiError(StatusCode::CONFLICT, e.to_string()))?;
    Ok(Json(find(&ctx, id.as_str())?.1.into()))
}

fn session_error(e: SessionError) -> ApiError {
    let code = match e {
        SessionError::NotFound => StatusCode::NOT_FOUND,
        SessionError::NotRunning | SessionError::NotInteractive => StatusCode::CONFLICT,
        SessionError::Unreachable(_) => StatusCode::BAD_GATEWAY,
        SessionError::SaveFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    ApiError(code, e.to_string())
}

async fn save(State(ctx): Ctx, Path(id): Path<String>) -> Result<Json<Saved>, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    let saved = session::save(&ctx, &id).await.map_err(session_error)?;
    Ok(Json(Saved { commits: saved.commits, branch: saved.branch }))
}

async fn close(State(ctx): Ctx, Path(id): Path<String>) -> Result<Json<TaskDto>, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    session::close(&ctx, &id).map_err(session_error)?;
    Ok(Json(find(&ctx, id.as_str())?.1.into()))
}

/// WebSocket ↔ terminal in the VM. From the browser: binary = input, JSON text `{"cols","rows"}` = resize.
async fn pty(
    State(ctx): Ctx,
    Path(id): Path<String>,
    Query(q): Query<PtyQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    if q.session != "claude" && q.session != "shell" {
        return Err(ApiError(StatusCode::BAD_REQUEST, "unknown session".into()));
    }
    let conn = session::open_terminal(&ctx, &id, &q.session, q.cols, q.rows, q.view).await.map_err(session_error)?;
    Ok(ws.on_upgrade(move |socket| bridge(socket, conn)))
}

async fn bridge(socket: WebSocket, conn: crate::app::session::Terminal) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (mut pty_rx, mut pty_tx) = conn.split();
    let to_browser = async {
        while let Ok(Some(bytes)) = pty_rx.recv().await {
            if ws_tx.send(Message::Binary(bytes.into())).await.is_err() {
                break;
            }
        }
        let _ = ws_tx.close().await;
    };
    let to_vm = async {
        while let Some(Ok(msg)) = ws_rx.next().await {
            let frame = match msg {
                Message::Binary(b) => TerminalInput::Input(b.to_vec()),
                Message::Text(t) => match serde_json::from_str::<serde_json::Value>(&t) {
                    Ok(v) => TerminalInput::Resize {
                        cols: v["cols"].as_u64().unwrap_or(120) as u16,
                        rows: v["rows"].as_u64().unwrap_or(36) as u16,
                    },
                    Err(_) => TerminalInput::Input(t.as_bytes().to_vec()),
                },
                Message::Close(_) => break,
                _ => continue,
            };
            if pty_tx.send(&frame).await.is_err() {
                break;
            }
        }
    };
    tokio::select! { _ = to_browser => {}, _ = to_vm => {} }
}

async fn asset(Path(file): Path<String>) -> Result<Response, ApiError> {
    let (body, mime): (&'static str, &str) = match file.as_str() {
        "app.css" => (include_str!("web/app.css"), "text/css"),
        "app.js" => (include_str!("web/app.js"), "text/javascript"),
        _ => return Err(ApiError(StatusCode::NOT_FOUND, "no such file".into())),
    };
    Ok(([("content-type", mime), ("cache-control", "no-cache")], body).into_response())
}

async fn font(file: &str) -> Option<Response> {
    let bytes: &'static [u8] = match file {
        "GeistVF.woff2" => include_bytes!("web/vendor/GeistVF.woff2"),
        "GeistMonoVF.woff2" => include_bytes!("web/vendor/GeistMonoVF.woff2"),
        _ => return None,
    };
    Some(([("content-type", "font/woff2"), ("cache-control", "max-age=604800")], bytes).into_response())
}

async fn vendor(Path(file): Path<String>) -> Result<Response, ApiError> {
    if let Some(font) = font(&file).await {
        return Ok(font);
    }
    let (body, mime): (&'static str, &str) = match file.as_str() {
        "xterm.js" => (include_str!("web/vendor/xterm.js"), "text/javascript"),
        "addon-fit.js" => (include_str!("web/vendor/addon-fit.js"), "text/javascript"),
        "addon-webgl.js" => (include_str!("web/vendor/addon-webgl.js"), "text/javascript"),
        "xterm.css" => (include_str!("web/vendor/xterm.css"), "text/css"),
        _ => return Err(ApiError(StatusCode::NOT_FOUND, "file not found".into())),
    };
    Ok(([("content-type", mime), ("cache-control", "max-age=86400")], body).into_response())
}

async fn logo() -> Response {
    ([("content-type", "image/svg+xml"), ("cache-control", "max-age=86400")], include_str!("web/logo.svg"))
        .into_response()
}

fn settings_view(ctx: &AppCtx) -> SettingsView {
    let settings = ctx.settings.get();
    let limits = ctx.settings.limits();
    SettingsView { recommended_max_vms: limits.recommended_vms(settings.memory_mb), settings, limits }
}

async fn get_settings(State(ctx): Ctx) -> Json<SettingsView> {
    Json(settings_view(&ctx))
}

async fn put_settings(State(ctx): Ctx, Json(new): Json<Settings>) -> Result<Json<SettingsView>, ApiError> {
    ctx.update_settings(new).map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(Json(settings_view(&ctx)))
}

async fn put_token(State(ctx): Ctx, Json(req): Json<TokenUpdate>) -> Result<StatusCode, ApiError> {
    let secret = Secret::new(req.token.trim().to_owned());
    let ctx2 = ctx.clone();
    tokio::task::spawn_blocking(move || ctx2.keychain.write_token(&secret))
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn golden_status(State(ctx): Ctx) -> Json<GoldenStatus> {
    Json(ctx.golden.status())
}

async fn golden_rebuild(State(ctx): Ctx) -> Result<Json<GoldenStatus>, ApiError> {
    let version = ctx.settings.get().claude_version;
    ctx.golden.rebuild(version.as_str()).map_err(|e| ApiError(StatusCode::CONFLICT, e.to_string()))?;
    Ok(Json(ctx.golden.status()))
}

async fn storage(State(ctx): Ctx) -> Json<StorageUsage> {
    Json(tokio::task::spawn_blocking(move || storage_usage(&ctx)).await.expect("storage scan"))
}

async fn storage_cleanup(State(ctx): Ctx) -> Json<serde_json::Value> {
    let removed = tokio::task::spawn_blocking(move || cleanup_finished_jobs(&ctx)).await.expect("cleanup");
    Json(serde_json::json!({ "removed": removed }))
}

async fn claude_versions(State(ctx): Ctx) -> Result<Json<crate::app::supervisor::ClaudeReleases>, ApiError> {
    ctx.claude_releases().await.map(Json).map_err(|e| ApiError(StatusCode::BAD_GATEWAY, e))
}

fn snapshot_error(e: crate::app::snapshots::SnapshotError) -> ApiError {
    use crate::app::snapshots::SnapshotError as E;
    let code = match e {
        E::NotFound | E::NoSnapshot => StatusCode::NOT_FOUND,
        E::NotRunning => StatusCode::CONFLICT,
        E::Submit(_) => StatusCode::BAD_REQUEST,
        E::SyncTimeout | E::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    ApiError(code, e.to_string())
}

fn snapshot_id(s: &str) -> Result<crate::domain::snapshot::SnapshotId, ApiError> {
    crate::domain::snapshot::SnapshotId::parse(s)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "snapshot not found".into()))
}

#[derive(serde::Deserialize, Default)]
struct SnapshotRequest {
    #[serde(default)]
    name: Option<String>,
}

async fn snapshot_task(
    State(ctx): Ctx,
    Path(id): Path<String>,
    body: Option<Json<SnapshotRequest>>,
) -> Result<Json<crate::domain::snapshot::SnapshotMeta>, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    let name = body.and_then(|Json(b)| b.name);
    crate::app::snapshots::take_snapshot(&ctx, &id, name).await.map(Json).map_err(snapshot_error)
}

async fn list_snapshots(State(ctx): Ctx) -> Json<Vec<crate::domain::snapshot::SnapshotMeta>> {
    Json(tokio::task::spawn_blocking(move || crate::app::snapshots::list(&ctx)).await.expect("list snapshots"))
}

async fn restore_snapshot(State(ctx): Ctx, Path(sid): Path<String>) -> Result<(StatusCode, Json<Created>), ApiError> {
    let sid = snapshot_id(&sid)?;
    let id = crate::app::snapshots::restore(&ctx, &sid).map_err(snapshot_error)?;
    Ok((StatusCode::CREATED, Json(Created { id: id.to_string() })))
}

async fn delete_snapshot(State(ctx): Ctx, Path(sid): Path<String>) -> Result<StatusCode, ApiError> {
    let sid = snapshot_id(&sid)?;
    crate::app::snapshots::delete(&ctx, &sid).map_err(snapshot_error)?;
    Ok(StatusCode::NO_CONTENT)
}

fn backup_error(e: crate::app::backups::BackupError) -> ApiError {
    use crate::app::backups::BackupError as E;
    let code = match e {
        E::NoSnapshot => StatusCode::NOT_FOUND,
        E::NotConfigured | E::Invalid(_) => StatusCode::BAD_REQUEST,
        E::S3(_) => StatusCode::BAD_GATEWAY,
        E::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    ApiError(code, e.to_string())
}

/// Streams the archive; the temporary file is unlinked once open.
async fn export_snapshot(State(ctx): Ctx, Path(sid): Path<String>) -> Result<Response, ApiError> {
    let sid = snapshot_id(&sid)?;
    let path = crate::app::backups::export(&ctx, &sid).await.map_err(backup_error)?;
    let file =
        tokio::fs::File::open(&path).await.map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let len = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    let _ = std::fs::remove_file(&path);
    let body = axum::body::Body::from_stream(tokio_util::io::ReaderStream::new(file));
    Ok((
        [
            ("content-type", "application/zstd".to_owned()),
            ("content-length", len.to_string()),
            ("content-disposition", format!("attachment; filename=\"{sid}.tar.zst\"")),
        ],
        body,
    )
        .into_response())
}

async fn import_snapshot(
    State(ctx): Ctx,
    body: axum::body::Body,
) -> Result<Json<crate::domain::snapshot::SnapshotMeta>, ApiError> {
    let internal = |e: std::io::Error| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    let dir = ctx.config.home.join("tmp");
    std::fs::create_dir_all(&dir).map_err(internal)?;
    let path = dir.join(format!(
        "upload-{}.tar.zst",
        crate::app::supervisor::random_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>()
    ));
    {
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::File::create(&path).await.map_err(internal)?;
        let mut stream = body.into_data_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
            file.write_all(&chunk).await.map_err(internal)?;
        }
        file.flush().await.map_err(internal)?;
    }
    let result = crate::app::backups::import(&ctx, &path).await;
    let _ = std::fs::remove_file(&path);
    result.map(Json).map_err(backup_error)
}

async fn backup_snapshot(
    State(ctx): Ctx,
    Path(sid): Path<String>,
) -> Result<Json<crate::app::backups::RemoteBackup>, ApiError> {
    let sid = snapshot_id(&sid)?;
    crate::app::backups::backup(&ctx, &sid).await.map(Json).map_err(backup_error)
}

async fn list_backups(State(ctx): Ctx) -> Result<Json<Vec<crate::app::backups::RemoteBackup>>, ApiError> {
    crate::app::backups::list(&ctx).await.map(Json).map_err(backup_error)
}

async fn restore_backup(
    State(ctx): Ctx,
    Path(sid): Path<String>,
) -> Result<Json<crate::domain::snapshot::SnapshotMeta>, ApiError> {
    let sid = snapshot_id(&sid)?;
    crate::app::backups::restore(&ctx, &sid).await.map(Json).map_err(backup_error)
}

async fn delete_backup(State(ctx): Ctx, Path(sid): Path<String>) -> Result<StatusCode, ApiError> {
    let sid = snapshot_id(&sid)?;
    crate::app::backups::delete(&ctx, &sid).await.map_err(backup_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Deserialize)]
struct S3Update {
    config: crate::domain::s3::S3Config,
    #[serde(default)]
    secret: Option<String>,
}

async fn get_s3(State(ctx): Ctx) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "config": ctx.settings.get().s3,
        "secret_saved": ctx.keychain.read_s3_secret().is_ok(),
    }))
}

async fn put_s3(State(ctx): Ctx, Json(req): Json<S3Update>) -> Result<StatusCode, ApiError> {
    crate::app::backups::configure_s3(&ctx, req.config, req.secret).await.map_err(backup_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn test_s3(State(ctx): Ctx) -> Result<StatusCode, ApiError> {
    crate::app::backups::test_s3(&ctx).await.map_err(backup_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_task(State(ctx): Ctx, Path(id): Path<String>) -> Result<StatusCode, ApiError> {
    use crate::app::queries::RemoveError;
    let (id, _) = find(&ctx, &id)?;
    crate::app::queries::remove_task(&ctx, &id).map_err(|e| {
        let code = match e {
            RemoveError::NotFound => StatusCode::NOT_FOUND,
            RemoveError::StillRunning => StatusCode::CONFLICT,
        };
        ApiError(code, e.to_string())
    })?;
    Ok(StatusCode::NO_CONTENT)
}

const UPLOAD_MAX: u64 = 2 << 30;

/// A file dropped on a terminal: `x-file-name` (URL-encoded) + the raw bytes. Returns its path in the VM.
async fn upload(State(ctx): Ctx, Path(id): Path<String>, req: Request) -> Result<Json<serde_json::Value>, ApiError> {
    use tokio::io::AsyncWriteExt;
    let (id, _) = find(&ctx, &id)?;
    let name = req
        .headers()
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .map(percent_decode)
        .ok_or_else(|| ApiError(StatusCode::BAD_REQUEST, "missing x-file-name".into()))?;
    let up = session::start_upload(&ctx, &id, &name).map_err(session_error)?;
    let mut file = tokio::fs::File::from_std(up.file);
    let mut body = req.into_body().into_data_stream();
    let mut written = 0u64;
    let result: Result<(), String> = async {
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            written += chunk.len() as u64;
            if written > UPLOAD_MAX {
                return Err("files over 2 GB cannot be dropped on a terminal".into());
            }
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        file.flush().await.map_err(|e| e.to_string())
    }
    .await;
    if let Err(e) = result {
        let _ = std::fs::remove_file(&up.host_path);
        return Err(ApiError(StatusCode::BAD_REQUEST, e));
    }
    Ok(Json(serde_json::json!({ "path": up.guest_path, "bytes": written })))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(b) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(b);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
