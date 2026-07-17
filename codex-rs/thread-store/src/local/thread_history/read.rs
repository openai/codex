use codex_protocol::ThreadId;
use codex_protocol::protocol::ThreadHistoryMode;
use serde::Deserialize;
use serde::Serialize;
use sqlx::QueryBuilder;
use sqlx::Row;
use sqlx::Sqlite;

use super::super::LocalThreadStore;
use super::thread_history_error;
use crate::ItemPage;
use crate::ListItemsParams;
use crate::ListTurnsParams;
use crate::SortDirection;
use crate::StoredThreadItem;
use crate::StoredTurn;
use crate::StoredTurnError;
use crate::StoredTurnItemsView;
use crate::StoredTurnStatus;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::TurnPage;

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryCursor {
    thread_id: ThreadId,
    scope: CursorScope,
    rollout_ordinal: i64,
    include_anchor: bool,
}

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(super) enum CursorScope {
    Turns,
    Items,
}

struct StoredTurnRow {
    turn_id: String,
    rollout_ordinal: i64,
    status: StoredTurnStatus,
    error: Option<StoredTurnError>,
    started_at: Option<i64>,
    completed_at: Option<i64>,
    duration_ms: Option<i64>,
    first_user_item_id: Option<String>,
    final_agent_item_id: Option<String>,
}

struct StoredThreadItemRow {
    item: StoredThreadItem,
    rollout_ordinal: i64,
}

pub(in crate::local) async fn list_turns(
    store: &LocalThreadStore,
    params: ListTurnsParams,
) -> ThreadStoreResult<TurnPage> {
    validate_thread_for_paginated_reads(
        store,
        params.thread_id,
        params.include_archived,
        "list_turns",
    )
    .await?;
    let scope = CursorScope::Turns;
    let cursor = parse_cursor(params.cursor.as_deref(), params.thread_id, &scope)?;
    let pool = store.thread_history_db().await?;
    let limit = page_limit(params.page_size)?;
    let mut query = QueryBuilder::<Sqlite>::new(
        r#"
SELECT
    turn_id,
    rollout_ordinal,
    status,
    error_json,
    started_at,
    completed_at,
    duration_ms,
    first_user_item_id,
    final_agent_item_id
FROM thread_turns
WHERE thread_id =
        "#,
    );
    query.push_bind(params.thread_id.to_string());
    push_pagination_clause(&mut query, params.sort_direction, cursor.as_ref(), limit);
    let rows = query
        .build()
        .fetch_all(pool)
        .await
        .map_err(thread_history_error)?;
    let mut turns = rows
        .into_iter()
        .map(stored_turn_row)
        .collect::<ThreadStoreResult<Vec<_>>>()?;
    let has_more = turns.len() > params.page_size;
    turns.truncate(params.page_size);

    let (next_cursor, backwards_cursor) = page_cursors(
        params.thread_id,
        &scope,
        turns.first().map(|turn| turn.rollout_ordinal),
        turns.last().map(|turn| turn.rollout_ordinal),
        has_more,
    )?;
    let mut stored_turns = Vec::with_capacity(turns.len());
    for turn in turns {
        let items = match params.items_view {
            StoredTurnItemsView::NotLoaded => Vec::new(),
            StoredTurnItemsView::Summary => {
                load_summary_items(pool, params.thread_id, &turn).await?
            }
        };
        stored_turns.push(StoredTurn {
            turn_id: turn.turn_id,
            items,
            items_view: params.items_view,
            status: turn.status,
            error: turn.error,
            started_at: turn.started_at,
            completed_at: turn.completed_at,
            duration_ms: turn.duration_ms,
        });
    }

    Ok(TurnPage {
        turns: stored_turns,
        next_cursor,
        backwards_cursor,
    })
}

pub(in crate::local) async fn list_items(
    store: &LocalThreadStore,
    params: ListItemsParams,
) -> ThreadStoreResult<ItemPage> {
    validate_thread_for_paginated_reads(
        store,
        params.thread_id,
        params.include_archived,
        "list_items",
    )
    .await?;
    let scope = CursorScope::Items;
    let cursor = parse_cursor(params.cursor.as_deref(), params.thread_id, &scope)?;
    let pool = store.thread_history_db().await?;
    let limit = page_limit(params.page_size)?;
    let mut query = QueryBuilder::<Sqlite>::new(
        r#"
SELECT turn_id, item_id, rollout_ordinal, created_at_ms, item_json
FROM thread_items
WHERE thread_id =
        "#,
    );
    query.push_bind(params.thread_id.to_string());
    if let Some(turn_id) = params.turn_id.as_deref() {
        query.push(" AND turn_id = ").push_bind(turn_id);
    }
    push_pagination_clause(&mut query, params.sort_direction, cursor.as_ref(), limit);
    let rows = query
        .build()
        .fetch_all(pool)
        .await
        .map_err(thread_history_error)?;
    let mut item_rows = rows
        .into_iter()
        .map(stored_thread_item_row)
        .collect::<ThreadStoreResult<Vec<_>>>()?;
    let has_more = item_rows.len() > params.page_size;
    item_rows.truncate(params.page_size);
    let (next_cursor, backwards_cursor) = page_cursors(
        params.thread_id,
        &scope,
        item_rows.first().map(|row| row.rollout_ordinal),
        item_rows.last().map(|row| row.rollout_ordinal),
        has_more,
    )?;
    let items = item_rows.into_iter().map(|row| row.item).collect();

    Ok(ItemPage {
        items,
        next_cursor,
        backwards_cursor,
    })
}

pub(super) async fn validate_thread_for_paginated_reads(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    include_archived: bool,
    operation: &'static str,
) -> ThreadStoreResult<()> {
    let Some(state_db) = store.state_db().await else {
        return Err(ThreadStoreError::Unsupported { operation });
    };
    let Some(metadata) =
        state_db
            .get_thread(thread_id)
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to read thread metadata: {err}"),
            })?
    else {
        return Err(ThreadStoreError::Unsupported { operation });
    };
    if metadata.archived_at.is_some() && !include_archived {
        return Err(ThreadStoreError::InvalidRequest {
            message: format!("thread {thread_id} is archived"),
        });
    }
    match metadata.history_mode {
        ThreadHistoryMode::Legacy => Err(ThreadStoreError::Unsupported { operation }),
        ThreadHistoryMode::Paginated => Ok(()),
    }
}

fn page_limit(page_size: usize) -> ThreadStoreResult<i64> {
    if page_size == 0 {
        return Err(ThreadStoreError::InvalidRequest {
            message: "page size must be positive".to_string(),
        });
    }
    let limit = page_size
        .checked_add(1)
        .ok_or_else(|| ThreadStoreError::InvalidRequest {
            message: "page size is too large".to_string(),
        })?;
    i64::try_from(limit).map_err(|_| ThreadStoreError::InvalidRequest {
        message: "page size is too large".to_string(),
    })
}

fn parse_cursor(
    cursor: Option<&str>,
    thread_id: ThreadId,
    scope: &CursorScope,
) -> ThreadStoreResult<Option<HistoryCursor>> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    let cursor_value: HistoryCursor =
        serde_json::from_str(cursor).map_err(|_| invalid_cursor(cursor))?;
    if cursor_value.thread_id != thread_id || &cursor_value.scope != scope {
        return Err(invalid_cursor(cursor));
    }
    Ok(Some(cursor_value))
}

fn push_pagination_clause(
    query: &mut QueryBuilder<Sqlite>,
    direction: SortDirection,
    cursor: Option<&HistoryCursor>,
    limit: i64,
) {
    if let Some(cursor) = cursor {
        let comparator = match (direction, cursor.include_anchor) {
            (SortDirection::Asc, true) => ">=",
            (SortDirection::Asc, false) => ">",
            (SortDirection::Desc, true) => "<=",
            (SortDirection::Desc, false) => "<",
        };
        query
            .push(" AND rollout_ordinal ")
            .push(comparator)
            .push(" ")
            .push_bind(cursor.rollout_ordinal);
    }
    let order = match direction {
        SortDirection::Asc => "ASC",
        SortDirection::Desc => "DESC",
    };
    query
        .push(" ORDER BY rollout_ordinal ")
        .push(order)
        .push(" LIMIT ")
        .push_bind(limit);
}

fn page_cursors(
    thread_id: ThreadId,
    scope: &CursorScope,
    first_ordinal: Option<i64>,
    last_ordinal: Option<i64>,
    has_more: bool,
) -> ThreadStoreResult<(Option<String>, Option<String>)> {
    let cursor = |rollout_ordinal, include_anchor| {
        serialize_cursor(thread_id, scope, rollout_ordinal, include_anchor)
    };
    let backwards_cursor = first_ordinal
        .map(|rollout_ordinal| cursor(rollout_ordinal, /*include_anchor*/ true))
        .transpose()?;
    let next_cursor = if has_more {
        last_ordinal
            .map(|rollout_ordinal| cursor(rollout_ordinal, /*include_anchor*/ false))
            .transpose()?
    } else {
        None
    };
    Ok((next_cursor, backwards_cursor))
}

pub(super) fn serialize_cursor(
    thread_id: ThreadId,
    scope: &CursorScope,
    rollout_ordinal: i64,
    include_anchor: bool,
) -> ThreadStoreResult<String> {
    serde_json::to_string(&HistoryCursor {
        thread_id,
        scope: scope.clone(),
        rollout_ordinal,
        include_anchor,
    })
    .map_err(thread_history_error)
}

fn invalid_cursor(cursor: &str) -> ThreadStoreError {
    ThreadStoreError::InvalidRequest {
        message: format!("invalid cursor: {cursor}"),
    }
}

fn stored_turn_row(row: sqlx::sqlite::SqliteRow) -> ThreadStoreResult<StoredTurnRow> {
    let status = match row.try_get::<String, _>("status")?.as_str() {
        "completed" => StoredTurnStatus::Completed,
        "interrupted" => StoredTurnStatus::Interrupted,
        "failed" => StoredTurnStatus::Failed,
        "inProgress" => StoredTurnStatus::InProgress,
        status => {
            return Err(ThreadStoreError::Internal {
                message: format!("unknown stored turn status: {status}"),
            });
        }
    };
    let error_json = row.try_get::<Option<String>, _>("error_json")?;
    let error = error_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(thread_history_error)?;
    Ok(StoredTurnRow {
        turn_id: row.try_get("turn_id")?,
        rollout_ordinal: row.try_get("rollout_ordinal")?,
        status,
        error,
        started_at: row.try_get("started_at")?,
        completed_at: row.try_get("completed_at")?,
        duration_ms: row.try_get("duration_ms")?,
        first_user_item_id: row.try_get("first_user_item_id")?,
        final_agent_item_id: row.try_get("final_agent_item_id")?,
    })
}

async fn load_summary_items(
    pool: &sqlx::SqlitePool,
    thread_id: ThreadId,
    turn: &StoredTurnRow,
) -> ThreadStoreResult<Vec<StoredThreadItem>> {
    let rows = sqlx::query(
        r#"
SELECT turn_id, item_id, rollout_ordinal, created_at_ms, item_json
FROM thread_items
WHERE thread_id = ?
  AND turn_id = ?
  AND (item_id = ? OR item_id = ?)
ORDER BY rollout_ordinal ASC
        "#,
    )
    .bind(thread_id.to_string())
    .bind(turn.turn_id.as_str())
    .bind(turn.first_user_item_id.as_deref())
    .bind(turn.final_agent_item_id.as_deref())
    .fetch_all(pool)
    .await
    .map_err(thread_history_error)?;
    rows.into_iter()
        .map(|row| stored_thread_item_row(row).map(|row| row.item))
        .collect()
}

fn stored_thread_item_row(row: sqlx::sqlite::SqliteRow) -> ThreadStoreResult<StoredThreadItemRow> {
    let rollout_ordinal = row.try_get::<i64, _>("rollout_ordinal")?;
    if rollout_ordinal < 0 {
        return Err(ThreadStoreError::Internal {
            message: format!("invalid stored item rollout ordinal: {rollout_ordinal}"),
        });
    }
    Ok(StoredThreadItemRow {
        item: StoredThreadItem {
            turn_id: row.try_get("turn_id")?,
            item_id: row.try_get("item_id")?,
            created_at_ms: row.try_get("created_at_ms")?,
            item_json: row.try_get::<String, _>("item_json")?.into_bytes(),
        },
        rollout_ordinal,
    })
}
