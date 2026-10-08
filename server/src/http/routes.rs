//! Route dell'API (spec §5).

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::{SinkExt, Stream, StreamExt};

use super::dto::{ApiError, CreateTask, Created, PtyQuery, Saved, Status, TaskDto};
use crate::app::session::{self, SessionError, TerminalInput};
use crate::app::events::StreamItem;
use crate::app::queries::{DiffError, task_diff};
use crate::app::store::TaskRecord;
use crate::app::supervisor::{AppCtx, NewTask, SubmitError, submit};
use crate::domain::ids::TaskId;

type Ctx = State<Arc<AppCtx>>;

pub fn router(ctx: Arc<AppCtx>) -> Router {
    Router::new()
        .route("/", get(dashboard))
        .route("/api/status", get(status))
        .route("/api/tasks", get(list).post(create))
        .route("/api/tasks/{id}", get(detail))
        .route("/api/tasks/{id}/events", get(events))
        .route("/api/tasks/{id}/diff", get(diff))
        .route("/api/tasks/{id}/stop", post(stop))
        .route("/api/tasks/{id}/save", post(save))
        .route("/api/tasks/{id}/close", post(close))
        .route("/api/tasks/{id}/pty", get(pty))
        .route("/vendor/{file}", get(vendor))
        .layer(middleware::from_fn_with_state(ctx.config.port, loopback_only))
        .with_state(ctx)
}

/// Difesa da DNS rebinding e da richieste cross-origin (POST e WebSocket): solo Host/Origin di loopback.
async fn loopback_only(State(port): State<u16>, req: Request, next: Next) -> Response {
    if is_loopback_request(port, &req) {
        next.run(req).await
    } else {
        ApiError(StatusCode::FORBIDDEN, "richiesta non consentita: usa http://127.0.0.1".into()).into_response()
    }
}

fn is_loopback_request(port: u16, req: &Request) -> bool {
    let allowed = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    let header = |name| req.headers().get(name).and_then(|v| v.to_str().ok());
    let host_ok = header(header::HOST).is_some_and(|h| allowed.iter().any(|a| a == h));
    let origin_ok = header(header::ORIGIN).is_none_or(|o| allowed.iter().any(|a| o == format!("http://{a}")));
    // Origin va controllato anche sui GET: un WebSocket cross-site verso un terminale è un GET.
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
        running: ctx.scheduler.running(),
    })
}

async fn create(State(ctx): Ctx, Json(req): Json<CreateTask>) -> Result<(StatusCode, Json<Created>), ApiError> {
    let new = NewTask {
        repo: &req.repo_path,
        prompt: req.prompt,
        base_ref: req.base_ref.as_deref(),
        interactive: req.interactive,
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
        .expect("eventi sempre serializzabili"))
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
        return Err(ApiError(StatusCode::CONFLICT, "il task è già terminato".into()));
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
    let commits = session::save(&ctx, &id).await.map_err(session_error)?;
    Ok(Json(Saved { commits }))
}

async fn close(State(ctx): Ctx, Path(id): Path<String>) -> Result<Json<TaskDto>, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    session::close(&ctx, &id).map_err(session_error)?;
    Ok(Json(find(&ctx, id.as_str())?.1.into()))
}

/// WebSocket ↔ terminale nella VM. Dal browser: binario = input, testo JSON `{"cols","rows"}` = resize.
async fn pty(
    State(ctx): Ctx,
    Path(id): Path<String>,
    Query(q): Query<PtyQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    if q.session != "claude" && q.session != "shell" {
        return Err(ApiError(StatusCode::BAD_REQUEST, "sessione sconosciuta".into()));
    }
    let conn = session::open_terminal(&ctx, &id, &q.session, q.cols, q.rows).await.map_err(session_error)?;
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

async fn vendor(Path(file): Path<String>) -> Result<Response, ApiError> {
    let (body, mime): (&'static str, &str) = match file.as_str() {
        "xterm.js" => (include_str!("web/vendor/xterm.js"), "text/javascript"),
        "addon-fit.js" => (include_str!("web/vendor/addon-fit.js"), "text/javascript"),
        "xterm.css" => (include_str!("web/vendor/xterm.css"), "text/css"),
        _ => return Err(ApiError(StatusCode::NOT_FOUND, "file inesistente".into())),
    };
    Ok(([("content-type", mime), ("cache-control", "max-age=86400")], body).into_response())
}
