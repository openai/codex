//! Reads immutable realtime snapshots by observation time or committed position.
//! Cursors retain the query and snapshot bound so late seals appear in catch-up reads.

use codex_app_server_protocol::ThreadRealtimeCoverage;
use codex_app_server_protocol::ThreadRealtimeEntry;
use codex_app_server_protocol::ThreadRealtimeItem;
use codex_app_server_protocol::ThreadRealtimeOrder;
use codex_protocol::ThreadId;
use codex_protocol::realtime::RealtimeItem;
use serde::Deserialize;
use serde::Serialize;
use sqlx::QueryBuilder;
use sqlx::Sqlite;

use super::super::LocalThreadStore;
use super::read::validate_thread_for_paginated_reads;
use super::sqlite_integer;
use super::thread_history_error;
use crate::ListRealtimeParams;
use crate::RealtimePage;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

const OBSERVED_AT: &str = "json_extract(item_json, '$.observed_at_ms')";

#[derive(Deserialize, Serialize)]
struct Cursor {
    version: u8,
    rollout_id: ThreadId,
    query: ListRealtimeParams,
    snapshot_end: u64,
    observed_at_ms: Option<i64>,
    position: u64,
}

pub(in crate::local) async fn list_realtime(
    store: &LocalThreadStore,
    params: ListRealtimeParams,
) -> ThreadStoreResult<RealtimePage> {
    if params.page_size == 0 || params.page_size > 500 {
        return Err(invalid("realtime page size must be between 1 and 500"));
    }
    if params
        .from_observed_at_ms
        .zip(params.to_observed_at_ms)
        .is_some_and(|(from, to)| from >= to)
    {
        return Err(invalid("realtime observation time range must be nonempty"));
    }
    match validate_thread_for_paginated_reads(
        store,
        params.thread_id,
        /*include_archived*/ true,
        "thread/realtime/list",
    )
    .await
    {
        Ok(()) => {}
        Err(ThreadStoreError::Unsupported { .. }) => {
            return Ok(empty_page(
                ThreadRealtimeCoverage::Unsupported,
                /*snapshot_end*/ 0,
                /*projected_through_item_index*/ 0,
            ));
        }
        Err(error) => return Err(error),
    }
    let _writer_guard = store.live_writer_locks.lock(params.thread_id).await;
    let resolved = super::super::thread_rollout_resolver::resolve_current_including_archived(
        store,
        params.thread_id,
    )
    .await?;
    let Some(resolved) = resolved else {
        return read_page(
            store.thread_history_db().await?,
            params.clone(),
            params.thread_id,
            /*first_position*/ 0,
            /*projected_end*/ 0,
        )
        .await;
    };
    let meta = codex_rollout::read_session_meta_line(&resolved.path)
        .await
        .map_err(thread_history_error)?
        .meta;
    let child_start = meta
        .subagent_history_start_ordinal
        .or_else(|| meta.history_base.map(|base| base.end_ordinal_exclusive));
    // A copied fork without a boundary cannot distinguish its own frames from the parent's.
    if meta.forked_from_id.is_some() && child_start.is_none() {
        return Ok(empty_page(
            ThreadRealtimeCoverage::Unsupported,
            /*snapshot_end*/ 0,
            /*projected_through_item_index*/ 0,
        ));
    }
    super::super::thread_history_materialization::materialize_to_sqlite(
        store,
        resolved.rollout_id,
        &resolved.path,
    )
    .await?;
    let pool = store.thread_history_db().await?;
    let projected_end = super::projection_state(store, resolved.rollout_id)
        .await?
        .map_or(0, |state| state.next_ordinal);
    read_page(
        pool,
        params,
        resolved.rollout_id,
        child_start.unwrap_or(0),
        projected_end,
    )
    .await
}

async fn read_page(
    pool: &sqlx::SqlitePool,
    params: ListRealtimeParams,
    rollout_id: ThreadId,
    first_position: u64,
    projected_end: u64,
) -> ThreadStoreResult<RealtimePage> {
    let mut query = params.clone();
    query.cursor = None;
    query.page_size = 0;
    // The effective snapshot is bound independently, so callers can omit it on later pages.
    query.snapshot_end = None;
    let cursor = params
        .cursor
        .as_deref()
        .map(serde_json::from_str::<Cursor>)
        .transpose()
        .map_err(|_| invalid("invalid realtime cursor"))?;
    if let Some(cursor) = &cursor
        && (cursor.version != 1
            || cursor.rollout_id != rollout_id
            || cursor.query != query
            || params
                .snapshot_end
                .is_some_and(|end| end != cursor.snapshot_end)
            || cursor.position < first_position
            || cursor.position >= cursor.snapshot_end)
    {
        return Err(invalid("realtime cursor does not match this query"));
    }
    let snapshot_end = cursor
        .as_ref()
        .map(|cursor| cursor.snapshot_end)
        .or(params.snapshot_end)
        .unwrap_or(projected_end);
    let exact_id_uses_current_snapshot =
        params.event_id.is_some() && cursor.is_none() && params.snapshot_end.is_none();
    if projected_end < snapshot_end
        || params
            .required_through_position
            .is_some_and(|position| projected_end <= position)
    {
        return Ok(empty_page(
            ThreadRealtimeCoverage::Pending,
            snapshot_end,
            projected_end,
        ));
    }
    let snapshot_end_sql = sqlite_integer(snapshot_end, "snapshot end")?;
    let mut sql = QueryBuilder::<Sqlite>::new(
        "SELECT rollout_ordinal, item_json FROM thread_realtime_items WHERE thread_id = ",
    );
    sql.push_bind(rollout_id.to_string());
    sql.push(" AND rollout_ordinal >= ")
        .push_bind(sqlite_integer(first_position, "first realtime position")?);
    // Only new, enrolled calls write a canonical timestamp.
    sql.push(" AND ")
        .push(OBSERVED_AT)
        .push(" IS NOT NULL AND item_type != 'transcript_segment'");
    if !exact_id_uses_current_snapshot {
        sql.push(" AND rollout_ordinal < ")
            .push_bind(snapshot_end_sql);
    }
    if let Some(after) = params.after_position {
        sql.push(" AND rollout_ordinal > ")
            .push_bind(sqlite_integer(after, "after position")?);
    }
    if let Some(session) = &params.session_id {
        sql.push(" AND json_extract(item_json, '$.realtime_session_id') = ")
            .push_bind(session);
    }
    if let Some(event_id) = &params.event_id {
        sql.push(" AND item_id = ").push_bind(event_id);
    }
    if let Some(from) = params.from_observed_at_ms {
        sql.push(" AND ")
            .push(OBSERVED_AT)
            .push(" >= ")
            .push_bind(from);
    }
    if let Some(to) = params.to_observed_at_ms {
        sql.push(" AND ")
            .push(OBSERVED_AT)
            .push(" < ")
            .push_bind(to);
    }
    if let Some(cursor) = &cursor {
        match params.order {
            ThreadRealtimeOrder::ObservedAt => {
                sql.push(" AND (")
                    .push(OBSERVED_AT)
                    .push(", rollout_ordinal) > (")
                    .push_bind(cursor.observed_at_ms)
                    .push(", ")
                    .push_bind(sqlite_integer(cursor.position, "cursor position")?)
                    .push(")");
            }
            ThreadRealtimeOrder::Position => {
                sql.push(" AND rollout_ordinal > ")
                    .push_bind(sqlite_integer(cursor.position, "cursor position")?);
            }
        }
    }
    sql.push(" ORDER BY ");
    if params.order == ThreadRealtimeOrder::ObservedAt {
        sql.push(OBSERVED_AT).push(", ");
    }
    sql.push("rollout_ordinal LIMIT ")
        .push_bind(i64::try_from(params.page_size + 1).map_err(thread_history_error)?);
    // Classify an exact-ID miss against the same projection snapshot as the page query.
    let mut transaction = pool.begin().await.map_err(thread_history_error)?;
    let rows = sql
        .build_query_as::<(i64, String)>()
        .fetch_all(&mut *transaction)
        .await
        .map_err(thread_history_error)?;
    // A second process can advance the projection after we capture its watermark.
    // An acknowledged ID outside that implicit snapshot is still waiting for coverage.
    if exact_id_uses_current_snapshot
        && rows
            .iter()
            .any(|(position, _)| *position >= snapshot_end_sql)
    {
        return Ok(empty_page(
            ThreadRealtimeCoverage::Pending,
            snapshot_end,
            projected_end,
        ));
    }
    let has_more = rows.len() > params.page_size;
    let data = rows
        .into_iter()
        .take(params.page_size)
        .map(|(position, json)| {
            Ok(ThreadRealtimeEntry {
                position: u64::try_from(position).map_err(thread_history_error)?,
                item: ThreadRealtimeItem::from_persisted(
                    serde_json::from_str::<RealtimeItem>(&json).map_err(thread_history_error)?,
                ),
            })
        })
        .collect::<ThreadStoreResult<Vec<_>>>()?;
    let next_cursor = if has_more {
        data.last()
            .map(|entry| {
                serde_json::to_string(&Cursor {
                    version: 1,
                    rollout_id,
                    query,
                    snapshot_end,
                    observed_at_ms: entry.item.observed_at_ms,
                    position: entry.position,
                })
                .map_err(thread_history_error)
            })
            .transpose()?
    } else {
        None
    };
    let coverage = if let Some(event_id) = &params.event_id
        && data.is_empty()
    {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM thread_realtime_items WHERE thread_id = ? AND item_id = ? AND rollout_ordinal >= ?)",
        )
        .bind(rollout_id.to_string())
        .bind(event_id)
        .bind(sqlite_integer(first_position, "first realtime position")?)
        .fetch_one(&mut *transaction)
        .await
        .map_err(thread_history_error)?;
        if exists {
            ThreadRealtimeCoverage::Ready
        } else {
            ThreadRealtimeCoverage::Pending
        }
    } else {
        ThreadRealtimeCoverage::Ready
    };
    transaction.commit().await.map_err(thread_history_error)?;
    Ok(RealtimePage {
        data,
        next_cursor,
        snapshot_end,
        projected_through_item_index: projected_end,
        coverage,
    })
}

fn empty_page(
    coverage: ThreadRealtimeCoverage,
    snapshot_end: u64,
    projected_through_item_index: u64,
) -> RealtimePage {
    RealtimePage {
        data: Vec::new(),
        next_cursor: None,
        snapshot_end,
        projected_through_item_index,
        coverage,
    }
}

fn invalid(message: &str) -> ThreadStoreError {
    ThreadStoreError::InvalidRequest {
        message: message.to_owned(),
    }
}

#[cfg(test)]
#[path = "realtime_stream_tests.rs"]
mod tests;
