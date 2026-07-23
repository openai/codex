use codex_protocol::ThreadId;
use codex_protocol::protocol::ThreadHistoryMode;
use serde::Deserialize;
use serde::Serialize;
use sqlx::Row;

use super::super::LocalThreadStore;
use super::segment_paging::page_item_rows;
use super::segment_paging::page_turn_rows;
use super::segment_paging::validate_page_size;
use crate::ItemPage;
use crate::ListItemsParams;
use crate::ListTurnsParams;
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

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(super) enum CursorScope {
    Turns,
    ItemsByCreatedAtOrdinal,
    ItemsByUpdatedAtOrdinal,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HistoryCursor {
    pub requested_thread_id: ThreadId,
    pub physical_thread_id: ThreadId,
    pub rollout_ordinal: u64,
    pub include_anchor: bool,
    pub scope: CursorScope,
}

#[derive(Clone, Copy)]
pub(super) struct PhysicalHistoryPosition {
    pub physical_thread_id: ThreadId,
    pub rollout_ordinal: i64,
}

pub(super) struct StoredTurnRow {
    pub position: PhysicalHistoryPosition,
    pub turn_id: String,
    pub status: StoredTurnStatus,
    pub error: Option<StoredTurnError>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub duration_ms: Option<i64>,
    pub first_user_item_id: Option<String>,
    pub final_agent_item_id: Option<String>,
}

pub(super) struct StoredThreadItemRow {
    pub position: PhysicalHistoryPosition,
    pub item: StoredThreadItem,
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
    validate_page_size(params.page_size)?;
    let lineage = store.resolve_rollout_lineage(params.thread_id).await?;
    let pool = store.thread_history_db().await?;
    let page = page_turn_rows(
        pool,
        params.thread_id,
        &lineage,
        params.cursor.as_deref(),
        params.page_size,
        params.sort_direction,
    )
    .await?;
    let mut turns = Vec::with_capacity(page.rows.len());
    for turn in page.rows {
        let items = match params.items_view {
            StoredTurnItemsView::NotLoaded => Vec::new(),
            StoredTurnItemsView::Summary => load_summary_items(pool, &turn).await?,
        };
        turns.push(StoredTurn {
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
        turns,
        next_cursor: page.next_cursor,
        backwards_cursor: page.backwards_cursor,
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
    validate_page_size(params.page_size)?;
    let lineage = store.resolve_rollout_lineage(params.thread_id).await?;
    let pool = store.thread_history_db().await?;
    let page = page_item_rows(pool, &lineage, &params).await?;

    Ok(ItemPage {
        items: page.rows.into_iter().map(|row| row.item).collect(),
        next_cursor: page.next_cursor,
        backwards_cursor: page.backwards_cursor,
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

async fn load_summary_items(
    pool: &sqlx::SqlitePool,
    turn: &StoredTurnRow,
) -> ThreadStoreResult<Vec<StoredThreadItem>> {
    let rows = sqlx::query(
        r#"
SELECT turn_id, item_id, updated_at_ordinal, created_at_ms, item_json
FROM thread_items
WHERE thread_id = ?
  AND turn_id = ?
  AND (item_id = ? OR item_id = ?)
ORDER BY rollout_ordinal ASC
        "#,
    )
    .bind(turn.position.physical_thread_id.to_string())
    .bind(turn.turn_id.as_str())
    .bind(turn.first_user_item_id.as_deref())
    .bind(turn.final_agent_item_id.as_deref())
    .fetch_all(pool)
    .await
    .map_err(super::thread_history_error)?;
    rows.into_iter().map(stored_thread_item).collect()
}

pub(super) fn parse_cursor(
    cursor: Option<&str>,
    requested_thread_id: ThreadId,
    scope: CursorScope,
) -> ThreadStoreResult<Option<HistoryCursor>> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    let cursor_value: HistoryCursor =
        serde_json::from_str(cursor).map_err(|_| invalid_cursor(cursor))?;
    if cursor_value.requested_thread_id != requested_thread_id || cursor_value.scope != scope {
        return Err(invalid_cursor(cursor));
    }
    Ok(Some(cursor_value))
}

pub(super) fn serialize_cursor(
    requested_thread_id: ThreadId,
    scope: CursorScope,
    position: PhysicalHistoryPosition,
    include_anchor: bool,
) -> ThreadStoreResult<String> {
    let rollout_ordinal = u64::try_from(position.rollout_ordinal)
        .map_err(|_| invalid_cursor("negative rollout ordinal"))?;
    serde_json::to_string(&HistoryCursor {
        requested_thread_id,
        physical_thread_id: position.physical_thread_id,
        rollout_ordinal,
        include_anchor,
        scope,
    })
    .map_err(super::thread_history_error)
}

pub(super) fn stored_turn_row(
    physical_thread_id: ThreadId,
    row: sqlx::sqlite::SqliteRow,
) -> ThreadStoreResult<StoredTurnRow> {
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
        .map_err(super::thread_history_error)?;
    Ok(StoredTurnRow {
        position: PhysicalHistoryPosition {
            physical_thread_id,
            rollout_ordinal: row.try_get("rollout_ordinal")?,
        },
        turn_id: row.try_get("turn_id")?,
        status,
        error,
        started_at: row.try_get("started_at")?,
        completed_at: row.try_get("completed_at")?,
        duration_ms: row.try_get("duration_ms")?,
        first_user_item_id: row.try_get("first_user_item_id")?,
        final_agent_item_id: row.try_get("final_agent_item_id")?,
    })
}

pub(super) fn stored_thread_item_row_for_thread(
    physical_thread_id: ThreadId,
    row: sqlx::sqlite::SqliteRow,
) -> ThreadStoreResult<StoredThreadItemRow> {
    let rollout_ordinal = row.try_get::<i64, _>("rollout_ordinal")?;
    if rollout_ordinal < 0 {
        return Err(ThreadStoreError::Internal {
            message: format!("invalid stored item rollout ordinal: {rollout_ordinal}"),
        });
    }
    Ok(StoredThreadItemRow {
        position: PhysicalHistoryPosition {
            physical_thread_id,
            rollout_ordinal,
        },
        item: stored_thread_item(row)?,
    })
}

fn stored_thread_item(row: sqlx::sqlite::SqliteRow) -> ThreadStoreResult<StoredThreadItem> {
    let updated_at_ordinal = row.try_get::<i64, _>("updated_at_ordinal")?;
    let updated_at_ordinal =
        u64::try_from(updated_at_ordinal).map_err(|_| ThreadStoreError::Internal {
            message: format!("invalid stored item updated-at ordinal: {updated_at_ordinal}"),
        })?;
    Ok(StoredThreadItem {
        turn_id: row.try_get("turn_id")?,
        item_id: row.try_get("item_id")?,
        updated_at_ordinal,
        created_at_ms: row.try_get("created_at_ms")?,
        item_json: row.try_get::<String, _>("item_json")?.into_bytes(),
    })
}

pub(super) fn invalid_cursor(cursor: &str) -> ThreadStoreError {
    ThreadStoreError::InvalidRequest {
        message: format!("invalid cursor: {cursor}"),
    }
}
