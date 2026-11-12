# Todo List

A single-user To Do List — one repository, the whole stack:

| folder | what | stack |
|---|---|---|
| [`server/`](server/) | REST API, and the production host for the client | Rust, [Axum](https://github.com/tokio-rs/axum), [rusqlite](https://github.com/rusqlite/rusqlite), SQLite |
| [`web/`](web/) | the client | [Astro](https://astro.build) (static build), TypeScript, vanilla DOM |
| [`docs/`](docs/) | documentation | [architecture](docs/architecture.md) · [API contract](docs/api.md) · [ADRs](docs/adr/) |

Glossary (todo, title, done, active, …): [CONTEXT.md](CONTEXT.md).

## Features

Add todos (title, optional description); complete and reopen them; delete; filter All / Active / Done; an "N active" counter. One flat list — no lists, due dates, or auth ([ADR 0003](docs/adr/0003-single-flat-list.md)).

## Quickstart

Both quickstarts run the server **from the repository root**, so its default paths (`data/todos.db`, `web/dist`) resolve without any environment variables.

### Development (two processes)

```sh
# terminal 1 — API on http://localhost:3000 (CORS enabled for :4321)
cargo run --manifest-path server/Cargo.toml

# terminal 2 — UI on http://localhost:4321, fetching :3000 directly
cd web && npm install && npm run dev
```

Open <http://localhost:4321>. (Prefer to work inside `server/`? `cd server && cargo run` works too — the database is simply created under `server/data/` in that case.)

### Production (one process)

```sh
# 1. build the client → web/dist
cd web && npm install && npm run build && cd ..

# 2. run: the same server serves the client AND the API on :3000
cargo run --release --manifest-path server/Cargo.toml
```

Open <http://localhost:3000>.

## Configuration

| env var | default | meaning |
|---|---|---|
| `TODO_ADDR` | `127.0.0.1:3000` | bind address |
| `TODO_DB_PATH` | `data/todos.db` | SQLite file, created on first start |
| `TODO_STATIC_DIR` | `web/dist` | built client, served for non-`/api` paths |

Defaults are relative to the directory the server is started from.

## Data

Your todos live in a SQLite file at `data/todos.db`, created on first start. Delete the file to start over; peek inside with the `sqlite3` CLI:

```sh
sqlite3 data/todos.db 'select * from todos;'
```

## Documentation

- [docs/architecture.md](docs/architecture.md) — layout, data flow, data model, toolchain
- [docs/api.md](docs/api.md) — the API contract (OpenAPI) both sides implement
- [docs/adr/](docs/adr/) — architecture decision records
- [CONTEXT.md](CONTEXT.md) — canonical glossary

## License

[MIT](LICENSE).


## Continuous integration

CI compiles/tests the existing Rust server without suppressing errors. The current baseline references missing server/src/state.rs and other unresolved implementation pieces. There is no web directory despite the README description, so no frontend build is claimed.

The workflows are read-only and do not deploy. Action dependencies are recorded in
`.github/workflows/actions.lock`. After changing action references, run
`gh actions-lock` and `gh actions-lock --verify` with official
`github/gh-actions-lock` v0.4.1, then commit the workflow and lockfile together.
