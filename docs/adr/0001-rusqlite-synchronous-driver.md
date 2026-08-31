# Synchronous rusqlite behind spawn_blocking, not an async driver

- Status: accepted (2026-08-30)
- M1 decision: yes (recorded for the future reader; the implementation lands in M2)

## Context

The API is async (Axum) but SQLite itself is synchronous. Options: (a) `tokio-rs-sqlite`, which wraps rusqlite to run queries off-thread; (b) `deadpool-sqlite`, the async wrapper for the `sqlite3` crate; (c) plain `rusqlite`. For a single-user list app the throughput need is trivial — the decision is about code shape and safety, not raw performance.

## Decision

Use **rusqlite** (with `--features bundled` for a self-contained static library) and keep exactly one `Connection` behind a `Mutex`. Every repository call is dispatched from the async handler through `tokio::spawn_blocking`, so the blocking I/O never runs on an async thread. The repository functions take `&Connection` (the `MutexGuard` is held only across the `spawn_blocking` hop and the query itself), which makes the data layer unit-testable with no runtime at all.

## Consequences

- **Pro** — no pool, no async-driver wrapper, no callback-shaped API: the data layer is three hundred lines of ordinary synchronous Rust. SQLite serializes writes through the database file anyway; a connection pool would buy us nothing at this scale and would add its own concurrency bugs.
- **Pro** — the `spawn_blocking` boundary is explicit at every call site, so "this blocks" is unmissable in review.
- **Con** — a burst of concurrent writes queues on the `Mutex`. Acceptable for one user; if it ever mattered, the escape hatch is a `SqlitePool` (deadpool) with minimal surface change, because the repository functions already look pool-like (plain `&Connection` args).
- **Con** — the tokio-rs-sqlite alternative disappears if we ever need `?blocking`-free ergonomics; revisit only with a real reason.

## Future readers

A switch away from rusqlite (e.g. to an async driver) is a decision to record as a **new ADR referencing this one** — it changes the concurrency story the rest of the architecture docs describe.
