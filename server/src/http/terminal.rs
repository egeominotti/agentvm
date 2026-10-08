//! A terminal in the VM over a WebSocket. From the browser: binary = input, JSON text
//! `{"cols","rows"}` = resize; to the browser: the terminal's output, as binary.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::response::Response;
use futures::{SinkExt, StreamExt};

use super::Ctx;
use super::dto::PtyQuery;
use super::error::ApiError;
use super::tasks::find;
use crate::app::session::{self, Terminal, TerminalInput};

pub async fn pty(
    State(ctx): Ctx,
    Path(id): Path<String>,
    Query(q): Query<PtyQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let (id, _) = find(&ctx, &id)?;
    if q.session != "claude" && q.session != "shell" {
        return Err(ApiError::bad_request("unknown session"));
    }
    let conn = session::open_terminal(&ctx, &id, &q.session, q.cols, q.rows, q.view).await?;
    Ok(ws.on_upgrade(move |socket| bridge(socket, conn)))
}

/// Pumps both ways until either side hangs up.
async fn bridge(socket: WebSocket, conn: Terminal) {
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
