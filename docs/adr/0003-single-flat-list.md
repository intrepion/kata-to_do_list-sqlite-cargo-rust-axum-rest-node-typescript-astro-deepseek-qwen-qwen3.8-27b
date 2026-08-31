# One flat list — no List entity

- Status: accepted (2026-08-30)
- M1 decision: yes (the shape of M2/M3; the README's "Features" section promises it)

## Context

"Todo list" apps conventionally nest todos under lists/projects. The feature set promised in the README is one flat collection of todos with a done flag, a filter, and a counter. Adding a `lists` table (foreign key, default-list bootstrap, list management UI, list-scoped routes) would multiply the schema, the API surface, and the client state for a feature nobody asked for.

## Decision

**One flat list.** The API is a single collection at `/api/todos`; there is no list resource, no foreign key, no grouping in the schema. "Active" and "Done" are *filters over the `done` flag*, not containers. The UI's All / Active / Done buttons and the `?status=` query parameter implement the filtering; "N active" counts `done = false`.

## Consequences

- **Pro** — one table, six routes, one client state object (`todos` + `filter`). Every code path in M2/M3 is directly exercised by the promised features; nothing is scaffolding.
- **Pro** — the data model stays a single SQL table, so ADR 0001's one-connection concurrency story holds without qualification.
- **Con** — users who want multiple lists cannot express that today. The escape hatch when (if) that becomes real: a `lists` table, a nullable `list_id` on `todos`, a default-list migration for existing rows, and list-scoped routes — all of that is a new ADR and a versioned API change, not a backward-compatible addition.

## Future readers

If you are reading this because someone asked for multiple lists, do not add the column quietly. Schema and API are M1 contracts; follow the ADR process and bump the API version.
