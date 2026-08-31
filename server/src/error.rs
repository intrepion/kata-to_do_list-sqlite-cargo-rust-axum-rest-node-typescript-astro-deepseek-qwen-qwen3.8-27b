//! Error type → HTTP mapping. One `AppError` for everything; the status
//! codes and human-readable messages come from `docs/api.md`.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use crate::db::DbError;

/// All the conditions an API request can fail with.
pub enum AppError {
    /// 400 — a field failed validation, or a query value is malformed.
    /// Carries the message that the client should show.
    Validation(String),
    /// 404 — the id does not exist (GET/PATCH/DELETE).
    NotFound,
    /// 405 — the path exists but the method does not. In practice Axum
    /// answers these on its own (plain-text body + `Allow` header); this
    /// variant exists so the mapping is complete and testable.
    MethodNotAllowed,
    /// 500 — anything else (notably `DbError::Internal`).
    Internal(String),
}

impl From<DbError> for AppError {
    fn from(e: DbError) -> Self {
        match e {
            DbError::NotFound => AppError::NotFound,
            DbError::Internal(msg) => AppError::Internal(msg),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Validation(message) => (StatusCode::BAD_REQUEST, message.clone()),
            AppError::NotFound => (StatusCode::NOT_FOUND, "todo not found".to_string()),
            AppError::MethodNotAllowed => (StatusCode::METHOD_NOT_ALLOWED, "method not allowed".to_string()),
            AppError::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message.clone()),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
