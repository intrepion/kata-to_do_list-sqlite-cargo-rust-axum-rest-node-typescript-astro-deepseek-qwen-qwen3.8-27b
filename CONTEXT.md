# Todo List — Language

Canonical glossary for this repo: a single-user To Do List web application (Rust/Axum REST API over SQLite, static Astro client). When one of these words appears in code, docs, or conversation, it means exactly what is written here.

## Language

**Todo**:
A single item of work the user is tracking. It has a required title, an optional description, and a completion state.
_Avoid_: task, entry

**Title**:
The required short name of a todo.
_Avoid_: name, label

**Description**:
An optional free-text note attached to a todo; it may be absent.
_Avoid_: body, comment

**Done**:
The state of a todo whose work is complete. A done todo stays in the list — completion never deletes it — and can be reopened.
_Avoid_: completed (as a noun), finished, closed

**Active**:
A todo that is not done.
_Avoid_: pending, open

**Reopen**:
The transition of a done todo back to active.
_Avoid_: uncomplete, undo

**List**:
The on-screen collection of all todos. Not a domain entity: there is no List object and no list id on a todo — all todos form one flat list (see `docs/adr/0003-single-flat-list.md`).
_Avoid_: board, folder, group
