//! `todo-server` binary — see `src/lib.rs` for the contract docs.
//!
//! Flags:
//!   --addr        listen address (default `127.0.0.1:8080`)
//!   --db          SQLite file path, created with parent dirs (default `data/todos.db`)
//!   --web-index   index.html of the built web app (default `../web/dist/index.html`)
//!
//! Dev vs release (docs/api.md "Non-API routes"): the mode follows the build
//! profile — `cargo run` (debug) serves the web index for non-API routes; a
//! release binary returns the JSON 404 envelope. No flag needed: `--help`
//! documents it, and it prints the chosen mode at startup.

use std::path::PathBuf;

use clap::Parser;
use todo_server::{
    handlers::SystemClock,
    state::AppState,
    Db,
};

#[derive(Parser, Debug)]
#[command(
    name = "todo-server",
    about = "Single-user To Do List REST API (Axum + SQLite). Non-API routes serve the web index in debug builds, JSON 404 in release builds."
)]
struct Args {
    /// Listen address.
    #[arg(long, default_value = "127.0.0.1:8080")]
    addr: String,

    /// SQLite database file (created if missing; parents created too).
    #[arg(short, long, default_value = "data/todos.db")]
    db: PathBuf,

    /// index.html of the built web app, served for non-API routes in debug builds.
    #[arg(long, default_value = "../web/dist/index.html")]
    web_index: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args = Args::parse();

    let dev = cfg!(debug_assertions);
    if dev {
        println!(
            "todo-server [dev]: non-API routes serve web index from {}",
            args.web_index.display()
        );
    } else {
        println!(
            "todo-server [release]: non-API routes return JSON 404 (web deploys separately)"
        );
    }

    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .try_init();

    if let Some(parent) = args.db.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("creating db dir {}: {e}", parent.display()))?;
        }
    }

    let db = Db::new(&args.db).map_err(|e| format!("opening {}: {e}", args.db.display()))?;
    println!(
        "sqlite ready: {} (journal_mode={}, foreign_keys={})",
        args.db.display(),
        db.pragma("journal_mode").await.unwrap_or_else(|e| e.to_string()),
        db.pragma("foreign_keys").await.unwrap_or_else(|e| e.to_string())
    );

    let state = AppState::new(db, SystemClock, args.web_index, dev);
    let app = todo_server::router(state);

    let listener = tokio::net::TcpListener::bind(&args.addr)
        .await
        .map_err(|e| format!("binding {addr}: {e}", addr = args.addr))?;
    println!("listening on http://{}/api/todos  (addr={})", args.addr, args.addr);

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("serve: {e}"))?;

    Ok(())
}
