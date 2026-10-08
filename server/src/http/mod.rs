//! HTTP interface: JSON API, SSE events and dashboard.
mod auth;
mod dto;
mod proxy;
mod routes;

pub use routes::router;
