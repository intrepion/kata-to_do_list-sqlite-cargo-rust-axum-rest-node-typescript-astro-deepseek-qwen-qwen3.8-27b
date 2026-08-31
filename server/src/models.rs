//! DTOs and enums — the JSON contract, field by field, per
//! `docs/architecture.md` → "JSON shapes".

use serde::{Deserialize, Serialize};

/// Field limits (architecture.md → "Validation"): titles 1..=200 chars,
/// descriptions 0..=2000 chars.
pub const TITLE_MAX: usize = 200;
pub const DESCRIPTION_MAX: usize = 2000;

/// Todo lifecycle. The single source of truth for the status string is
/// `as_str()` / `parse()`; the database stores the same two literals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TodoStatus {
    Active,
    Done,
}

impl TodoStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TodoStatus::Active => "active",
            TodoStatus::Done => "done",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "active" => Some(TodoStatus::Active),
            "done" => Some(TodoStatus::Done),
            _ => None,
        }
    }
}

/// Wire representation of a stored todo. `description` is `None` for a
/// cleared/absent description (JSON: field omitted, not `null`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Todo {
    pub id: i64,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: TodoStatus,
    /// RFC 3339 UTC, second precision: `YYYY-MM-DDTHH:MM:SSZ` (ADR-0003).
    pub created_at: String,
    pub updated_at: String,
}

/// `POST /api/todos`. `description` omitted = none.
#[derive(Debug, Clone, Deserialize)]
pub struct TodoCreate {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// `PATCH /api/todos/:id`. **Presence is the signal** (architecture.md): a
/// missing key keeps the stored value; a present key — including explicit
/// `null` for the description — sets it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TodoUpdate {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TodoStatus>,
}

/// `GET /api/todos?filter=`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StatusFilter {
    #[default]
    All,
    Active,
    Done,
}

impl StatusFilter {
    /// The only spelling accepted is lowercase; anything else is a 400.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "all" => Some(StatusFilter::All),
            "active" => Some(StatusFilter::Active),
            "done" => Some(StatusFilter::Done),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn todo_status_serde_is_lowercase() {
        assert_eq!(serde_json::to_string(&TodoStatus::Active).unwrap(), "\"active\"");
        assert_eq!(serde_json::to_string(&TodoStatus::Done).unwrap(), "\"done\"");
        assert_eq!(serde_json::from_str::<TodoStatus>("\"done\"").unwrap(), TodoStatus::Done);
        assert!(serde_json::from_str::<TodoStatus>("\"DONE\"").is_err());
    }

    #[test]
    fn todo_serializes_without_none_description() {
        let todo = Todo {
            id: 1,
            title: "t".into(),
            description: None,
            status: TodoStatus::Active,
            created_at: "2026-07-22T00:00:00Z".into(),
            updated_at: "2026-07-22T00:00:00Z".into(),
        };
        let v = serde_json::to_value(&todo).unwrap();
        assert!(v.get("description").is_none());
        assert_eq!(v["status"], "active");
    }

    #[test]
    fn todo_deserializes_missing_description_as_none() {
        let todo: Todo = serde_json::from_value(json!({"id":1,"title":"t","status":"active","created_at":"c","updated_at":"u"})).unwrap();
        assert!(todo.description.is_none());
    }

    #[test]
    fn todo_update_defaults_are_none() {
        let u: TodoUpdate = serde_json::from_value(json!({})).unwrap();
        assert!(u.title.is_none() && u.description.is_none() && u.status.is_none());
    }

    #[test]
    fn todo_update_explicit_null_description_stays_none_but_present() {
        // Both the omitted key and the explicit null land in `None` here;
        // db.rs distinguishes them with contains_key (presence, ADR-0004).
        let u: TodoUpdate = serde_json::from_value(json!({"description": null})).unwrap();
        assert!(u.description.is_none());
    }

    #[test]
    fn status_filter_parse() {
        assert_eq!(StatusFilter::parse("all"), Some(StatusFilter::All));
        assert_eq!(StatusFilter::parse("active"), Some(StatusFilter::Active));
        assert_eq!(StatusFilter::parse("done"), Some(StatusFilter::Done));
        assert_eq!(StatusFilter::parse("DONE"), None);
        assert_eq!(StatusFilter::parse("bogus"), None);
        assert_eq!(StatusFilter::parse(""), None);
    }

    #[test]
    fn status_filter_defaults_to_all() {
        assert_eq!(StatusFilter::default(), StatusFilter::All);
    }
}
