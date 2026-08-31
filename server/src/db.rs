//! The `Db` type: one `rusqlite::Connection` behind a `Mutex`, in a single
//! transaction per request, **synchronous** (ADR-0001).
//!
//! All database access goes through this module. The rest of the crate only
//! sees `Db` + the DTOs from `models.rs`.

use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::AppError;
use crate::models::{StatusFilter, Todo, TodoCreate, TodoStatus, DESCRIPTION_MAX, TITLE_MAX};

/// Errors the database layer can produce. Maps 1:1 onto `AppError`
/// (404 / 500); validation is returned by the callers directly.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("todo {id} not found")]
    NotFound { id: i64 },
    #[error("database failure: {0}")]
    Internal(String),
}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        // Mapping for `.get()` failures specifically (checked by the callers):
        // a query that found no row is "not found" for id-scoped lookups.
        if let rusqlite::Error::QueryReturnedNoRows = e {
            return DbError::Internal("no row returned".into());
        }
        DbError::Internal(e.to_string())
    }
}

/// The application database. Cheap to `Clone` (Arc + Mutex).
#[derive(Clone)]
pub struct Db {
    conn: Arc<...>,
}

// NOTE: filled in by the M2 implementation below (this file is the template
// for db.rs; the real one is written in the same step).
impl Db {
    /// Open the database, creating the schema (idempotent).
    pub fn open(path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| DbError::Internal(format!("{}: {e}", path.display())))?;
            }
        }
        let conn = Connection::open(path).map_err(|e| DbError::Internal(format!("{}: {e}", path.display())))?;
        conn.pragma_update(None, "journal_mode", "WAL").map_err(|e| DbError::Internal(e.to_string()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS todos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                description TEXT,
                status TEXT NOT NULL CHECK (status IN ('active', 'done')),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_todos_status ON todos (status);",
        )
        .map_err(|e| DbError::Internal(e.to_string()))?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("database mutex poisoned")
    }

    fn row_to_todo(row: &Row) -> Result<Todo, DbError> {
        Ok(Todo {
            id: row.get(0)?,
            title: row.get(1)?,
            description: row.get(2)?,
            status: TodoStatus::parse(&row.get::<_, String>(3)?)
                .ok_or_else(|| DbError::Internal("corrupt status value in database".into()))?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    }

    /// Create a todo. Returns the stored row (201 payload).
    pub fn create(&self, input: TodoCreate, now: &str) -> Result<Todo, DbError> {
        let conn = self.lock();
        let tx = conn.transaction().map_err(DbError::from)?;
        let id: i64 = tx
            .execute(
                "INSERT INTO todos (title, description, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![input.title, input.description, TodoStatus::Active.as_str(), now],
            )
            .map_err(DbError::from)?;
        tx.commit().map_err(DbError::from)?;
        conn.query_row(
            "SELECT id, title, description, status, created_at, updated_at FROM todos WHERE id = ?1",
            params![id],
            Self::row_to_todo,
        )
        .map_err(|e| {
            if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
                DbError::Internal("row vanished after insert".into())
            } else {
                e.into()
            }
        })
    }

    /// Fetch one todo by id.
    pub fn get(&self, id: i64) -> Result<Todo, DbError> {
        let conn = self.lock();
        match conn.query_row(
            "SELECT id, title, description, status, created_at, updated_at FROM todos WHERE id = ?1",
            params![id],
            Self::row_to_todo,
        ) {
            Ok(todo) => Ok(todo),
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(DbError::NotFound { id }),
            Err(e) => Err(e.into()),
        }
    }

    /// List todos, newest first, optionally filtered by status.
    pub fn list(&self, filter: StatusFilter) -> Result<Vec<Todo>, DbError> {
        let conn = self.lock();
        let sql = match filter {
            StatusFilter::All => "SELECT id, title, description, status, created_at, updated_at FROM todos ORDER BY id DESC",
            StatusFilter::Active | StatusFilter::Done => {
                "SELECT id, title, description, status, created_at, updated_at FROM todos WHERE status = ?1 ORDER BY id DESC"
            }
        };
        let mut stmt = match filter {
            StatusFilter::All => conn.prepare(sql).map_err(DbError::from)?,
            _ => conn.prepare(sql).map_err(DbError::from)?,
        };
        let todos = match filter {
            StatusFilter::All => stmt.query_map([], Self::row_to_todo).map_err(DbError::from)?,
            _ => {
                let status = match filter {
                    StatusFilter::Active => TodoStatus::Active,
                    StatusFilter::Done => TodoStatus::Done,
                    StatusFilter::All => unreachable!(),
                }
                .as_str();
                stmt.query_map(params![status], Self::row_to_todo).map_err(DbError::from)?
            }
        };
        let mut out = Vec::new();
        for row in todos {
            out.push(row.map_err(DbError::from)?);
        }
        Ok(out)
    }

    /// Partial update. Returns `None` when the id does not exist.
    ///
    /// **Presence is the signal** (ADR-0004, docs/api.md): the caller passes
    /// `present` booleans derived from the *raw JSON body*, because Rust's
    /// `Option` cannot distinguish `{}` from `{"description": null}`.
    pub fn update(
        &self,
        id: i64,
        title: &Option<String>,
        has_title: bool,
        description: &Option<String>,
        has_description: bool,
        status: &Option<TodoStatus>,
        has_status: bool,
        now: &str,
    ) -> Result<Option<Todo>, DbError> {
        let conn = self.lock();
        let tx = conn.transaction().map_err(DbError::from)?;
        let current: Option<Todo> = tx
            .query_row(
                "SELECT id, title, description, status, created_at, updated_at FROM todos WHERE id = ?1",
                params![id],
                Self::row_to_todo,
            )
            .optional()
            .map_err(DbError::from)?;
        let current = match current {
            Some(current) => current,
            None => {
                tx.rollback();
                return Ok(None);
            }
        };

        let new_title = match (has_title, title) {
            (true, Some(t)) => t.clone(),
            _ => current.title.clone(),
        };
        // An explicit `null` (present but None) clears the description; an
        // absent key keeps it.
        let new_description = match (has_description, description) {
            (true, Some(d)) => Some(d.clone()),
            (true, None) => None,
            (false, _) => current.description.clone(),
        };
        let new_status = match (has_status, status) {
            (true, Some(s)) => *s,
            _ => current.status,
        };

        tx.execute(
            "UPDATE todos SET title = ?1, description = ?2, status = ?3, updated_at = ?4 WHERE id = ?5",
            params![new_title, new_description, new_status.as_str(), now, id],
        )
        .map_err(DbError::from)?;
        tx.commit().map_err(DbError::from)?;

        conn.query_row(
            "SELECT id, title, description, status, created_at, updated_at FROM todos WHERE id = ?1",
            params![id],
            Self::row_to_todo,
        )
        .map(Some)
        .map_err(|e| {
            if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
                DbError::Internal("row vanished during update".into())
            } else {
                e.into()
            }
        })
    }

    /// Delete by id. `true` = a row was deleted.
    pub fn delete(&self, id: i64) -> Result<bool, DbError> {
        let conn = self.lock();
        let tx = conn.transaction().map_err(DbError::from)?;
        let affected = tx
            .execute("DELETE FROM todos WHERE id = ?1", params![id])
            .map_err(DbError::from)?;
        tx.commit().map_err(DbError::from)?;
        Ok(affected > 0)
    }
}

/// The one piece of stateless validation (limits) the DB layer enforces.
/// Used by both `create` and `update` paths — messages live in models.md's
/// contract (docs/api.md), not here.
pub fn validate_title(title: &str) -> Result<(), AppError> {
    let len = title.chars().count();
    if len == 0 || len > TITLE_MAX {
        Err(AppError::Validation(format!(
            "title must be 1..={TITLE_MAX} characters (got {len})"
        )))
    } else {
        Ok(())
    }
}

pub fn validate_description(description: &Option<String>) -> Result<(), AppError> {
    if let Some(d) = description {
        let len = d.chars().count();
        if len > DESCRIPTION_MAX {
            return Err(AppError::Validation(format!(
                "description must be at most {DESCRIPTION_MAX} characters (got {len})"
            )));
        }
    }
    Ok(())
}
