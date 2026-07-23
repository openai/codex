use codex_protocol::ThreadId;
use sqlx::QueryBuilder;
use sqlx::Sqlite;

use super::super::rollout_lineage::RolloutLineage;
use super::super::rollout_lineage::RolloutLineageSegment;
use super::read::CursorScope;
use super::read::HistoryCursor;
use super::read::PhysicalHistoryPosition;
use super::read::StoredThreadItemRow;
use super::read::StoredTurnRow;
use super::read::invalid_cursor;
use super::read::parse_cursor;
use super::read::serialize_cursor;
use super::read::stored_thread_item_row_for_thread;
use super::read::stored_turn_row;
use super::thread_history_error;
use crate::ItemSortKey;
use crate::ListItemsParams;
use crate::SortDirection;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) struct SegmentPage<T> {
    pub rows: Vec<T>,
    pub next_cursor: Option<String>,
    pub backwards_cursor: Option<String>,
}

pub(super) fn validate_page_size(page_size: usize) -> ThreadStoreResult<()> {
    if page_size == 0 {
        return Err(ThreadStoreError::InvalidRequest {
            message: "page size must be positive".to_string(),
        });
    }
    let limit = page_size.checked_add(1).ok_or_else(page_size_too_large)?;
    i64::try_from(limit).map_err(|_| page_size_too_large())?;
    Ok(())
}

pub(super) async fn page_turn_rows(
    pool: &sqlx::SqlitePool,
    requested_thread_id: ThreadId,
    lineage: &RolloutLineage,
    cursor: Option<&str>,
    page_size: usize,
    direction: SortDirection,
) -> ThreadStoreResult<SegmentPage<StoredTurnRow>> {
    let cursor = parse_cursor(cursor, requested_thread_id, CursorScope::Turns)?;
    let mut rows = Vec::new();
    for (segment, segment_cursor) in segments_from_cursor(lineage, direction, cursor.as_ref())? {
        let remaining = remaining_limit(page_size, rows.len())?;
        if remaining == 0 {
            break;
        }
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
        query.push_bind(segment.thread_id().to_string());
        push_segment_range(&mut query, segment)?;
        push_cursor_clause(&mut query, direction, segment_cursor)?;
        push_order_and_limit(&mut query, direction, remaining);
        rows.extend(
            query
                .build()
                .fetch_all(pool)
                .await
                .map_err(thread_history_error)?
                .into_iter()
                .map(|row| stored_turn_row(segment.thread_id(), row))
                .collect::<ThreadStoreResult<Vec<_>>>()?,
        );
    }
    finish_page(requested_thread_id, CursorScope::Turns, rows, page_size)
}

pub(super) async fn page_item_rows(
    pool: &sqlx::SqlitePool,
    lineage: &RolloutLineage,
    params: &ListItemsParams,
) -> ThreadStoreResult<SegmentPage<StoredThreadItemRow>> {
    // Update ordinals are local to a physical rollout. Forked lineages need a structured
    // watermark before incremental replay can safely span their segments.
    if params.after_updated_at_ordinal.is_some() && lineage.segments().len() > 1 {
        return Err(ThreadStoreError::InvalidRequest {
            message: "incremental item replay is not supported for forked threads".to_string(),
        });
    }
    if matches!(params.sort_key, ItemSortKey::UpdatedAtOrdinal) {
        let Some(after_updated_at_ordinal) = params.after_updated_at_ordinal else {
            return Err(ThreadStoreError::InvalidRequest {
                message: "update-ordinal item sorting requires an update watermark".to_string(),
            });
        };
        return page_updated_item_rows(pool, params, after_updated_at_ordinal).await;
    }
    let cursor = parse_cursor(
        params.cursor.as_deref(),
        params.thread_id,
        CursorScope::ItemsByCreatedAtOrdinal,
    )?;
    let mut rows = Vec::new();
    for (segment, segment_cursor) in
        segments_from_cursor(lineage, params.sort_direction, cursor.as_ref())?
    {
        let remaining = remaining_limit(params.page_size, rows.len())?;
        if remaining == 0 {
            break;
        }
        let mut query = QueryBuilder::<Sqlite>::new(
            r#"
SELECT turn_id, item_id, rollout_ordinal, updated_at_ordinal, created_at_ms, item_json
FROM thread_items
WHERE thread_id =
            "#,
        );
        query.push_bind(segment.thread_id().to_string());
        push_segment_range(&mut query, segment)?;
        if let Some(after_updated_at_ordinal) = params.after_updated_at_ordinal {
            query
                .push(" AND updated_at_ordinal > ")
                .push_bind(sqlite_integer(after_updated_at_ordinal)?);
        }
        if let Some(turn_id) = params.turn_id.as_deref() {
            query.push(" AND turn_id = ").push_bind(turn_id);
        }
        push_cursor_clause(&mut query, params.sort_direction, segment_cursor)?;
        push_order_and_limit(&mut query, params.sort_direction, remaining);
        rows.extend(
            query
                .build()
                .fetch_all(pool)
                .await
                .map_err(thread_history_error)?
                .into_iter()
                .map(|row| stored_thread_item_row_for_thread(segment.thread_id(), row))
                .collect::<ThreadStoreResult<Vec<_>>>()?,
        );
    }
    finish_page(
        params.thread_id,
        CursorScope::ItemsByCreatedAtOrdinal,
        rows,
        params.page_size,
    )
}

async fn page_updated_item_rows(
    pool: &sqlx::SqlitePool,
    params: &ListItemsParams,
    after_updated_at_ordinal: u64,
) -> ThreadStoreResult<SegmentPage<StoredThreadItemRow>> {
    let cursor = parse_cursor(
        params.cursor.as_deref(),
        params.thread_id,
        CursorScope::ItemsByUpdatedAtOrdinal,
    )?;
    if cursor
        .as_ref()
        .is_some_and(|cursor| cursor.physical_thread_id != params.thread_id)
    {
        return Err(invalid_cursor("unknown physical segment"));
    }
    let mut query = QueryBuilder::<Sqlite>::new(
        r#"
SELECT turn_id, item_id, updated_at_ordinal AS rollout_ordinal, updated_at_ordinal, created_at_ms, item_json
FROM thread_items
WHERE thread_id =
        "#,
    );
    query
        .push_bind(params.thread_id.to_string())
        .push(" AND updated_at_ordinal > ")
        .push_bind(sqlite_integer(after_updated_at_ordinal)?);
    if let Some(turn_id) = params.turn_id.as_deref() {
        query.push(" AND turn_id = ").push_bind(turn_id);
    }
    if let Some(cursor) = cursor {
        let comparator = match (params.sort_direction, cursor.include_anchor) {
            (SortDirection::Asc, true) => ">=",
            (SortDirection::Asc, false) => ">",
            (SortDirection::Desc, true) => "<=",
            (SortDirection::Desc, false) => "<",
        };
        query
            .push(" AND updated_at_ordinal ")
            .push(comparator)
            .push(" ")
            .push_bind(sqlite_integer(cursor.rollout_ordinal)?);
    }
    let order = match params.sort_direction {
        SortDirection::Asc => "ASC",
        SortDirection::Desc => "DESC",
    };
    query
        .push(" ORDER BY updated_at_ordinal ")
        .push(order)
        .push(" LIMIT ")
        .push_bind(remaining_limit(params.page_size, /*row_count*/ 0)?);
    let rows = query
        .build()
        .fetch_all(pool)
        .await
        .map_err(thread_history_error)?
        .into_iter()
        .map(|row| stored_thread_item_row_for_thread(params.thread_id, row))
        .collect::<ThreadStoreResult<Vec<_>>>()?;
    finish_page(
        params.thread_id,
        CursorScope::ItemsByUpdatedAtOrdinal,
        rows,
        params.page_size,
    )
}

fn segments_from_cursor<'a>(
    lineage: &'a RolloutLineage,
    direction: SortDirection,
    cursor: Option<&'a HistoryCursor>,
) -> ThreadStoreResult<Vec<(&'a RolloutLineageSegment, Option<&'a HistoryCursor>)>> {
    let segments = lineage.segments();
    let cursor_index = cursor
        .map(|cursor| {
            segments
                .iter()
                .position(|segment| segment.thread_id() == cursor.physical_thread_id)
                .ok_or_else(|| invalid_cursor("unknown physical segment"))
        })
        .transpose()?;
    if let Some(cursor) = cursor
        && let Some(index) = cursor_index
        && !cursor_in_segment(cursor, &segments[index])
    {
        return Err(invalid_cursor("position outside physical segment"));
    }
    let indexes: Vec<usize> = match direction {
        SortDirection::Asc => (cursor_index.unwrap_or(0)..segments.len()).collect(),
        SortDirection::Desc => {
            let end = cursor_index.unwrap_or_else(|| segments.len().saturating_sub(1));
            (0..=end).rev().collect()
        }
    };
    Ok(indexes
        .into_iter()
        .map(|index| {
            let segment_cursor = if Some(index) == cursor_index {
                cursor
            } else {
                None
            };
            (&segments[index], segment_cursor)
        })
        .collect())
}

fn cursor_in_segment(cursor: &HistoryCursor, segment: &RolloutLineageSegment) -> bool {
    let ordinal = cursor.rollout_ordinal;
    ordinal >= segment.start_ordinal()
        && segment
            .end_ordinal()
            .is_none_or(|end_ordinal| ordinal < end_ordinal)
}

fn push_segment_range(
    query: &mut QueryBuilder<Sqlite>,
    segment: &RolloutLineageSegment,
) -> ThreadStoreResult<()> {
    query
        .push(" AND rollout_ordinal >= ")
        .push_bind(sqlite_integer(segment.start_ordinal())?);
    if let Some(end_ordinal) = segment.end_ordinal() {
        query
            .push(" AND rollout_ordinal < ")
            .push_bind(sqlite_integer(end_ordinal)?);
    }
    Ok(())
}

fn push_cursor_clause(
    query: &mut QueryBuilder<Sqlite>,
    direction: SortDirection,
    cursor: Option<&HistoryCursor>,
) -> ThreadStoreResult<()> {
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
            .push_bind(sqlite_integer(cursor.rollout_ordinal)?);
    }
    Ok(())
}

fn push_order_and_limit(query: &mut QueryBuilder<Sqlite>, direction: SortDirection, limit: i64) {
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

fn remaining_limit(page_size: usize, row_count: usize) -> ThreadStoreResult<i64> {
    let limit = page_size
        .checked_add(1)
        .and_then(|limit| limit.checked_sub(row_count))
        .ok_or_else(page_size_too_large)?;
    i64::try_from(limit).map_err(|_| page_size_too_large())
}

fn finish_page<T: HasPosition>(
    requested_thread_id: ThreadId,
    scope: CursorScope,
    mut rows: Vec<T>,
    page_size: usize,
) -> ThreadStoreResult<SegmentPage<T>> {
    let has_more = rows.len() > page_size;
    rows.truncate(page_size);
    let backwards_cursor = rows
        .first()
        .map(|row| {
            serialize_cursor(
                requested_thread_id,
                scope.clone(),
                row.position(),
                /*include_anchor*/ true,
            )
        })
        .transpose()?;
    let next_cursor = if has_more {
        rows.last()
            .map(|row| {
                serialize_cursor(
                    requested_thread_id,
                    scope,
                    row.position(),
                    /*include_anchor*/ false,
                )
            })
            .transpose()?
    } else {
        None
    };
    Ok(SegmentPage {
        rows,
        next_cursor,
        backwards_cursor,
    })
}

trait HasPosition {
    fn position(&self) -> PhysicalHistoryPosition;
}

impl HasPosition for StoredTurnRow {
    fn position(&self) -> PhysicalHistoryPosition {
        self.position
    }
}

impl HasPosition for StoredThreadItemRow {
    fn position(&self) -> PhysicalHistoryPosition {
        self.position
    }
}

fn sqlite_integer(value: u64) -> ThreadStoreResult<i64> {
    i64::try_from(value).map_err(|_| ThreadStoreError::InvalidRequest {
        message: "rollout ordinal exceeds SQLite integer range".to_string(),
    })
}

fn page_size_too_large() -> ThreadStoreError {
    ThreadStoreError::InvalidRequest {
        message: "page size is too large".to_string(),
    }
}
