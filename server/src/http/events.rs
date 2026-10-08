//! A task's live events as Server-Sent Events: state changes and the agent's output, each with its
//! sequence number as the event id.

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::{Stream, StreamExt};

use super::Ctx;
use super::error::ApiError;
use super::tasks::find;
use crate::app::events::StreamItem;

pub async fn events(
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
