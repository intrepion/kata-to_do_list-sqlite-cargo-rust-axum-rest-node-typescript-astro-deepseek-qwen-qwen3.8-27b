//! `todo-server` — the single-user To Do List REST API (Axum + SQLite).
//!
//! API contract: [`../docs/api.md`](file://$ROOT/docs/api.md),
//! validation rules: [`../docs/spec.md`](file://$ROOT/docs/spec.md),
//! error-shape rationale: ADR-0002, timestamps: ADR-0003.
//!
//! Modules:
//! - [`db`] — SQLite schema + queries (single connection, `PRAGMA journal_mode=WAL`,
//!   `PRAGMA foreign_keys=ON`);
//! - [`models`] — request/response shapes + validation (title 1..=200 chars,
//!   description 0..=2000, nullable);
//! - [`handlers`] — the five endpoints, all of `docs/api.md`;
//! - [`error`] — the fixed JSON error envelope (ADR-0002);
//! - [`state`] — shared application state (DB + clock + web index + mode);
//! - [`router`] — full router: `/api/*` + CORS + fallbacks.
//!
//! Non-API routes (docs/api.md): in **dev** builds they serve the built web
//! app's `index.html` (so the M3 browser build can hit the API same-origin);
//! in **release** builds they return the JSON 404 envelope (web is *not*
//! embedded — it deploys separately, docs/design.md §Deployment).

pub mod db;
pub mod error;
pub mod handlers;
pub mod models;
pub mod state;

use axum::routing::{delete, get, patch, post, put};
use axum::Router;
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;

pub use db::Db;
use handlers::{api_not_found, dev_fallback, method_not_allowed};
use state::AppState;

/// Build the full application router (API + CORS + fallbacks), parameterized
/// over shared state; call `.with_state(state)` before serving.
///
/// Exposed (not only in `main`) so the integration suite (`tests/api.rs`) can
/// exercise the exact same wiring the binary serves.
pub fn router(state: AppState) -> Router {
    // Every method on every API path is registered: the allowed ones run the
    // real handler, the others produce the JSON 405 envelope (docs/api.md
    // "Status codes") — Axum's built-in 405 is HTML, which the contract
    // forbids.
    let api = Router::new()
        .route(
            "/api/todos",
            get(handlers::list)
                .post(handlers::create)
                .put(method_not_allowed)
                .patch(method_not_allowed)
                .delete(method_not_allowed),
        )
        .route(
            "/api/todos/{id}",
            get(handlers::get)
                .patch(handlers::update)
                .delete(handlers::delete)
                .post(method_not_allowed)
                .put(method_not_allowed),
        )
        // Unknown /api/* → JSON 404 (never HTML, docs/api.md).
        .fallback(api_not_found)
        // M2: permissive CORS for the Astro dev server; the M3 same-origin
        // browser build makes this a no-op in production paths.
        .layer(CorsLayer::permissive());

    Router::new()
        .nest("/api", api)
        // Non-API routes: dev → web index.html; release → JSON 404 envelope.
        .fallback(dev_fallback)
        .with_state(state)
}

/// JSON error envelope, fixed shape (ADR-0002).
pub fn error_envelope(code: &str, message: &str) -> Json<Value> {
    Json(json!({ "error": { "code", message } }))
}
