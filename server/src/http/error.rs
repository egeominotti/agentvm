//! The API's single error type: a status and a message for people, sent as `{"error": "..."}`.
//! Every app error picks its HTTP status here, once, so handlers just use `?`.

use std::fmt::Display;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::app::backups::BackupError;
use crate::app::queries::{DiffError, RemoveError};
use crate::app::session::SessionError;
use crate::app::snapshots::SnapshotError;
use crate::app::supervisor::SubmitError;

pub struct ApiError(pub StatusCode, pub String);

impl ApiError {
    pub fn new(code: StatusCode, message: impl Display) -> Self {
        ApiError(code, message.to_string())
    }

    /// No task with that id (or not even an id).
    pub fn not_found() -> Self {
        ApiError::new(StatusCode::NOT_FOUND, "task not found")
    }

    pub fn bad_request(message: impl Display) -> Self {
        ApiError::new(StatusCode::BAD_REQUEST, message)
    }

    pub fn conflict(message: impl Display) -> Self {
        ApiError::new(StatusCode::CONFLICT, message)
    }

    pub fn internal(message: impl Display) -> Self {
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({ "error": self.1 }))).into_response()
    }
}

impl From<SubmitError> for ApiError {
    fn from(e: SubmitError) -> Self {
        let code = match e {
            SubmitError::NoGolden(_) | SubmitError::Token(_) => StatusCode::SERVICE_UNAVAILABLE,
            SubmitError::DiskFull(_) => StatusCode::INSUFFICIENT_STORAGE,
            _ => StatusCode::BAD_REQUEST,
        };
        ApiError::new(code, e)
    }
}

impl From<DiffError> for ApiError {
    fn from(e: DiffError) -> Self {
        let code = match e {
            DiffError::NotFound => StatusCode::NOT_FOUND,
            DiffError::NoBranch => StatusCode::CONFLICT,
            DiffError::Git(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(code, e)
    }
}

impl From<RemoveError> for ApiError {
    fn from(e: RemoveError) -> Self {
        let code = match e {
            RemoveError::NotFound => StatusCode::NOT_FOUND,
            RemoveError::StillRunning => StatusCode::CONFLICT,
        };
        ApiError::new(code, e)
    }
}

impl From<SessionError> for ApiError {
    fn from(e: SessionError) -> Self {
        let code = match e {
            SessionError::NotFound => StatusCode::NOT_FOUND,
            SessionError::NotRunning | SessionError::NotInteractive => StatusCode::CONFLICT,
            SessionError::Unreachable(_) => StatusCode::BAD_GATEWAY,
            SessionError::SaveFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            SessionError::CloseFailed(_) | SessionError::SnapshotFailed(_) => StatusCode::CONFLICT,
        };
        ApiError::new(code, e)
    }
}

impl From<SnapshotError> for ApiError {
    fn from(e: SnapshotError) -> Self {
        use SnapshotError as E;
        let code = match e {
            E::NotFound | E::NoSnapshot => StatusCode::NOT_FOUND,
            E::NotRunning | E::InUse => StatusCode::CONFLICT,
            E::Submit(_) | E::Interval => StatusCode::BAD_REQUEST,
            E::SyncTimeout | E::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            E::DiskFull(_) => StatusCode::INSUFFICIENT_STORAGE,
        };
        ApiError::new(code, e)
    }
}

impl From<BackupError> for ApiError {
    fn from(e: BackupError) -> Self {
        use BackupError as E;
        let code = match e {
            E::NoSnapshot => StatusCode::NOT_FOUND,
            E::NotConfigured | E::Invalid(_) => StatusCode::BAD_REQUEST,
            E::S3(_) => StatusCode::BAD_GATEWAY,
            E::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            E::DiskFull(_) => StatusCode::INSUFFICIENT_STORAGE,
        };
        ApiError::new(code, e)
    }
}
