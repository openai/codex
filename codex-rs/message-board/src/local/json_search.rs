//! Literal search over decoded JSON fields in a fixed read view.

use super::StoredPost;
use super::paging::Window;
use super::storage_error;
use caseless::default_case_fold_str;
use codex_protocol::error::Result;
use futures::TryStreamExt;
use serde_json::Value;
use sqlx::QueryBuilder;
use sqlx::Sqlite;
use sqlx::SqliteConnection;

pub(super) async fn page(
    mut sql: QueryBuilder<Sqlite>,
    conn: &mut SqliteConnection,
    pointer: &str,
    needle: &str,
    window: &Window,
) -> Result<Vec<StoredPost>> {
    // Plain text matches the folded column directly. Also consider rows in which
    // JSON escaping could hide a match, including Unicode escapes for ASCII text.
    let escape = if needle
        .bytes()
        .any(|b| b < 0x20 || matches!(b, b'"' | b'/' | b'\\'))
    {
        r"\"
    } else {
        r"\u"
    };
    sql.push(" AND (instr(body_search,")
        .push_bind(needle)
        .push(")>0 OR instr(body_search,")
        .push_bind(escape)
        .push(")>0) ORDER BY timestamp DESC,seq DESC");
    let mut rows = sql.build_query_scalar::<String>().fetch(conn);
    let mut skip = window.offset();
    let mut posts = Vec::new();
    while let Some(row) = rows.try_next().await.map_err(storage_error)? {
        let post: StoredPost = serde_json::from_str(&row).map_err(storage_error)?;
        if !matches_post(&post.text, pointer, needle) {
            continue;
        }
        if skip > 0 {
            skip -= 1;
            continue;
        }
        posts.push(post);
        if posts.len() > window.limit {
            break;
        }
    }
    Ok(posts)
}

fn matches_post(text: &str, pointer: &str, needle: &str) -> bool {
    if default_case_fold_str(text).contains(needle) {
        return true;
    }
    let Ok(post) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    // The arguments string is searchable both as posted JSON and as decoded values.
    contains_literal(&post, needle)
        || post
            .pointer(pointer)
            .and_then(Value::as_str)
            .and_then(|raw| serde_json::from_str(raw).ok())
            .is_some_and(|arguments| contains_literal(&arguments, needle))
}

fn contains_literal(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(text) => default_case_fold_str(text).contains(needle),
        Value::Array(items) => items.iter().any(|item| contains_literal(item, needle)),
        Value::Object(fields) => fields.iter().any(|(key, item)| {
            default_case_fold_str(key).contains(needle) || contains_literal(item, needle)
        }),
        Value::Number(_) | Value::Bool(_) | Value::Null => value.to_string().contains(needle),
    }
}
