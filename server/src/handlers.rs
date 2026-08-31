//! HTTP handlers. Thin layer over `Db`: parse → validate → call `Db` →
//! `AppError` mapping. All timestamps come from `now` (the state's clock —
//! injectable in tests).

use std::time::SystemTime;

use axum::extract::{Path as UrlPath, Query, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::AppError;
use crate::models::{StatusFilter, Todo, TodoCreate, TodoStatus};
use crate::state::AppState;
use crate::Db;

const JSON: &str = "application/json";

/// The one clock the server uses. `now_iso()` renders it in the documented
/// second-precision RFC 3339 shape (ADR-0003).
#[derive(Clone, Copy)]
pub struct SystemClock;

pub trait Clock {
    fn now_iso(&self) -> String;
}

impl Clock for SystemClock {
    fn now_iso(&self) -> String {
        let secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format_ts(secs)
    }
}

/// Render a Unix-seconds instant in the documented shape: RFC 3339 UTC with
/// second precision, e.g. `2026-07-22T15:04:05Z` (ADR-0003). Exposed so
/// tests can assert exact timestamps with a fake clock.
pub fn format_ts(secs: u64) -> String {
    chrono::DateTime::from_timestamp(secs as i64, 0)
        .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

// ---------------------------------------------------------------------------
// request parsers (raw JSON for presence semantics — ADR-0004)
// ---------------------------------------------------------------------------

/// `POST /api/todos` body. `description` is `Option` — omitted or `null`
/// means "no description"; presence of the *key* only matters for PATCH.
#[derive(Debug, Deserialize)]
struct CreateBody {
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

/// `PATCH /api/todos/:id` body. Raw `Value` first: **presence of a key in
/// the JSON object is the signal** (ADR-0004) — `None` alone cannot tell
/// `{"description": null}` apart from `{}`.
fn parse_update_body(raw: &Value) -> Result<(Option<String>, bool, Option<String>, bool, Option<TodoStatus>, bool), AppError> {
    let obj = raw
        .as_object()
        .ok_or_else(|| AppError::Validation("body must be a JSON object".into()))?;

    let (title, has_title) = match obj.get("title") {
        None => (None, false),
        Some(Value::Null) => Err(AppError::Validation(
            "title must be a string (omit the key to leave it unchanged)".into(),
        ))?,
        Some(Value::String(s)) => (Some(s.clone()), true),
        Some(_) => Err(AppError::Validation(
            "title must be a string (omit the key to leave it unchanged)".into(),
        ))?,
    };

    // `null` is a first-class value for description: it clears it.
    let (description, has_description) = match obj.get("description") {
        None => (None, false),
        Some(Value::Null) => (None, true),
        Some(Value::String(s)) => (Some(s.clone()), true),
        Some(_) => Err(AppError::Validation(
            "description must be a string or null".into(),
        ))?,
    };

    let (status, has_status) = match obj.get("status") {
        None => (None, false),
        Some(Value::String(s)) => match TodoStatus::parse(s) {
            Some(s) => (Some(s), true),
            None => Err(AppError::Validation(format!(
                "status must be \"active\" or \"done\" (got {s:?})"
            )))?,
        },
        Some(Value::Null) => Err(AppError::Validation(
            "status must be \"active\" or \"done\" (omit the key to leave it unchanged)".into(),
        ))?,
        Some(_) => Err(AppError::Validation(
            "status must be \"active\" or \"done\" (omit the key to leave it unchanged)".into(),
        ))?,
    };

    Ok((title, has_title, description, has_description, status, has_status))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    filter: Option<String>,
}

// ---------------------------------------------------------------------------
// handlers
// ---------------------------------------------------------------------------

/// `GET /api/todos?filter=all|active|done`
pub async fn list(State(state): State<AppState>, Query(q): Query<ListQuery>) -> Result<Json<Vec<Todo>>, AppError> {
    let filter = match &q.filter {
        None => StatusFilter::All,
        Some(s) => StatusFilter::parse(s)
            .ok_or_else(|| AppError::Validation(format!("filter must be \"all\", \"active\", or \"done\" (got {s:?})")))?,
    };
    let todos = state.db.list(filter).map_err(AppError::from)?;
    Ok(Json(todos))
}

/// `POST /api/todos` → 201 + the created todo.
pub async fn create(State(state): State<AppState>, Json(body): Json<CreateBody>) -> Result<(axum::http::StatusCode, Json<Todo>), AppError> {
    let title = body
        .title
        .filter(|t| !t.is_empty()) // `None` and `""` → missing/empty, same error
        .ok_or_else(|| AppError::Validation("title is required".into()))?;
    crate::db::validate_title(&title)?;
    crate::db::validate_description(&body.description)?;
    let input = TodoCreate {
        title: title.into(),
        description: body.description.clone(),
    };
    let now = state.clock.now_iso();
    let todo = state.db.create(input, &now).map_err(AppError::from)?;
    Ok((axum::http::StatusCode::CREATED, Json(todo)))
}

/// `GET /api/todos/:id` → 200 + the todo, or 404.
pub async fn get(State(state): State<AppState>, UrlPath(id): UrlPath<i64>) -> Result<Json<Todo>, AppError> {
    Ok(Json(state.db.get(id).map_err(AppError::from)?))
}

/// `PATCH /api/todos/:id` → 200 + the updated todo, or 400/404.
pub async fn update(State(state): State<AppState>, UrlPath(id): UrlPath<i64>, Json(raw): Json<Value>) -> Result<Json<Todo>, AppError> {
    let (title, has_title, description, has_description, status, has_status) = parse_update_body(&raw)?;
    if has_title {
        crate::db::validate_title(title.as_deref().unwrap_or(""))?;
    }
    if has_description {
        crate::db::validate_description(&description)?;
    }
    if !has_title && !has_description && !has_status {
        return Err(AppError::Validation(
            "at least one of \"title\", \"description\", or \"status\" is required".into(),
        ));
    }
    let now = state.clock.now_iso();
    let updated = state
        .db
        .update(id, &title, has_title, &description, has_description, &status, has_status, &now)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound { id })?;
    Ok(Json(updated))
}

/// `DELETE /api/todos/:id` → 204, or 404.
pub async fn delete(State(state): State<AppState>, UrlPath(id): UrlPath<i64>) -> Result<axum::http::StatusCode, AppError> {
    if state.db.delete(id).map_err(AppError::from)? {
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound { id })
    }
}

// ---------------------------------------------------------------------------
// fallbacks (registered in lib.rs)
// ---------------------------------------------------------------------------

/// Unknown `/api/...` path → JSON 404 (not the HTML dev fallback).
pub fn api_not_found() -> (axum::http::StatusCode, &'static str, Json<Value>) {
    (
        axum::http::StatusCode::NOT_FOUND,
        JSON,
        Json(json!({"error": {"code": "not_found", "message": "route not found"}})),
    )
}

/// The catch-all for **non-API** routes (registered at `/`, GET/HEAD only,
/// in lib.rs): in development, serve the built web app's `index.html` so
/// client-side routing works; in release, an explanatory JSON 404.
///
/// Method filtering is the router's job: a `POST /` never reaches this
/// handler (the route is GET/HEAD-only), so there is no method logic here.
pub async fn dev_fallback(State(state): State<AppState>) -> Response {
    if !state.dev {
        return (
            axum::http::StatusCode::NOT_FOUND,
            JSON,
            Json(json!({"error": {"code": "not_found", "message": "not found"}})),
        )
            .into_response();
    }
    match std::fs::read_to_string(&state.web_index) {
        Ok(html) => (
            axum::http::StatusCode::OK,
            [("content-type", "text/html; charset=utf-8")],
            html,
        )
            .into_response(),
        Err(e) => (
            axum::http::StatusCode::NOT_FOUND,
            JSON,
            Json(json!({
                "error": {
                    "code": "web_not_built",
                    "message": format!("could not read {}: {e}. Run `npm --prefix web run build` first.", state.web_index.display())
                }
            })),
        )
            .into_response(),
    }
}
