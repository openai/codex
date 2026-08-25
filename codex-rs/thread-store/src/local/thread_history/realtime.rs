use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadTimelineEntry;
use codex_protocol::ThreadId;
use codex_protocol::realtime::RealtimeItem;
use serde::Deserialize;
use serde::Serialize;

use super::super::LocalThreadStore;
use super::read::validate_thread_for_paginated_reads;
use super::segment_paging::validate_page_size;
use super::sqlite_integer;
use super::thread_history_error;
use crate::ListTimelineParams;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::TimelinePage;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TimelineCursor {
    thread_id: ThreadId,
    position: u64,
    kind: u8,
    id: String,
}

// A rollout record can materialize both an item and a turn boundary. Keep the
// boundary order stable, including when the page cuts through one ordinal.
pub(super) fn entry_key(entry: &ThreadTimelineEntry) -> (u64, u8, &str) {
    match entry {
        ThreadTimelineEntry::TurnStarted {
            position, turn_id, ..
        } => (*position, 0, turn_id),
        ThreadTimelineEntry::Item { position, item, .. } => (*position, 1, item.id()),
        ThreadTimelineEntry::Realtime { position, item } => (*position, 2, &item.id),
        ThreadTimelineEntry::TurnCompleted {
            position, turn_id, ..
        } => (*position, 3, turn_id),
    }
}

pub(in crate::local) async fn list_timeline(
    store: &LocalThreadStore,
    params: ListTimelineParams,
) -> ThreadStoreResult<TimelinePage> {
    validate_thread_for_paginated_reads(
        store,
        params.thread_id,
        /*include_archived*/ false,
        "thread/timeline/list",
    )
    .await?;
    validate_page_size(params.page_size)?;

    let lineage = store.resolve_rollout_lineage(params.thread_id).await?;
    let pool = store.thread_history_db().await?;
    let cursor = params
        .cursor
        .as_deref()
        .map(serde_json::from_str::<TimelineCursor>)
        .transpose()
        .map_err(|_| ThreadStoreError::InvalidRequest {
            message: "invalid thread timeline cursor".to_string(),
        })?
        .map(|cursor| {
            if cursor.thread_id != params.thread_id {
                Err(ThreadStoreError::InvalidRequest {
                    message: "thread timeline cursor belongs to another thread".to_string(),
                })
            } else {
                Ok(cursor)
            }
        })
        .transpose()?;
    let cursor_position = cursor.as_ref().map(|cursor| cursor.position);
    let cursor_ordinal = cursor_position
        .map(|position| sqlite_integer(position, "rollout ordinal"))
        .transpose()?
        .unwrap_or(i64::MAX);
    let cursor_kind = cursor.as_ref().map_or(4, |cursor| cursor.kind);
    let cursor_id = cursor.as_ref().map_or("", |cursor| cursor.id.as_str());

    let mut rows = Vec::new();
    let max_rows = params.page_size + 1;
    for segment in lineage.segments().iter().rev() {
        let remaining = max_rows.saturating_sub(rows.len());
        if remaining == 0 {
            break;
        }
        if cursor_position.is_some_and(|position| position < segment.start_ordinal()) {
            continue;
        }
        let upper = segment
            .end_ordinal()
            .map(|value| sqlite_integer(value, "rollout ordinal"))
            .transpose()?
            .unwrap_or(i64::MAX);
        let segment_rows = sqlx::query_as::<_, (i64, i64, String, Option<String>, String)>(
            r#"
WITH starts AS (
SELECT rollout_ordinal, 0 AS kind, turn_id AS id, turn_id,
       json_object('type', 'turnStarted', 'position', rollout_ordinal,
                   'turnId', turn_id, 'startedAt', started_at) AS item_json
FROM thread_turns
WHERE thread_id = ?1 AND rollout_ordinal >= ?2
  AND rollout_ordinal < ?3 AND rollout_ordinal <= ?4
  AND (rollout_ordinal, 0, turn_id) < (?4, ?5, ?6)
ORDER BY rollout_ordinal DESC, turn_id DESC LIMIT ?7
), items AS (
SELECT rollout_ordinal, 1 AS kind, item_id AS id, turn_id, item_json
FROM thread_items
WHERE thread_id = ?1 AND rollout_ordinal >= ?2
  AND rollout_ordinal < ?3 AND rollout_ordinal <= ?4
  AND (rollout_ordinal, 1, item_id) < (?4, ?5, ?6)
ORDER BY rollout_ordinal DESC, item_id DESC LIMIT ?7
), realtime AS (
SELECT rollout_ordinal, 2 AS kind, item_id AS id, NULL AS turn_id, item_json
FROM thread_realtime_items
WHERE thread_id = ?1 AND rollout_ordinal >= ?2
  AND rollout_ordinal < ?3 AND rollout_ordinal <= ?4
  AND (rollout_ordinal, 2, item_id) < (?4, ?5, ?6)
ORDER BY rollout_ordinal DESC, item_id DESC LIMIT ?7
), ends AS (
SELECT rollout_end_ordinal AS rollout_ordinal, 3 AS kind, turn_id AS id, turn_id,
       json_object('type', 'turnCompleted', 'position', rollout_end_ordinal,
                   'turnId', turn_id, 'status', status, 'error', json(error_json),
                   'startedAt', started_at, 'completedAt', completed_at,
                   'durationMs', duration_ms) AS item_json
FROM thread_turns
WHERE thread_id = ?1 AND rollout_end_ordinal >= ?2
  AND rollout_end_ordinal < ?3 AND rollout_end_ordinal <= ?4
  AND (rollout_end_ordinal, 3, turn_id) < (?4, ?5, ?6)
ORDER BY rollout_end_ordinal DESC, turn_id DESC LIMIT ?7
)
SELECT * FROM starts
UNION ALL SELECT * FROM items
UNION ALL SELECT * FROM realtime
UNION ALL SELECT * FROM ends
ORDER BY rollout_ordinal DESC, kind DESC, id DESC
LIMIT ?7
            "#,
        )
        .bind(segment.rollout_id().to_string())
        .bind(sqlite_integer(segment.start_ordinal(), "rollout ordinal")?)
        .bind(upper)
        .bind(cursor_ordinal)
        .bind(i64::from(cursor_kind))
        .bind(cursor_id)
        .bind(i64::try_from(remaining).map_err(thread_history_error)?)
        .fetch_all(pool)
        .await
        .map_err(thread_history_error)?;

        for (position, kind, _id, turn_id, item_json) in segment_rows {
            let position = u64::try_from(position).map_err(thread_history_error)?;
            rows.push(match kind {
                0 | 3 => serde_json::from_str::<ThreadTimelineEntry>(&item_json)
                    .map_err(thread_history_error)?,
                1 => ThreadTimelineEntry::Item {
                    position,
                    turn_id: turn_id.ok_or_else(|| thread_history_error("missing turn ID"))?,
                    item: Box::new(
                        serde_json::from_str::<ThreadItem>(&item_json)
                            .map_err(thread_history_error)?,
                    ),
                },
                2 => ThreadTimelineEntry::Realtime {
                    position,
                    item: serde_json::from_str::<RealtimeItem>(&item_json)
                        .map_err(thread_history_error)?
                        .into(),
                },
                _ => return Err(thread_history_error("invalid timeline entry kind")),
            });
        }
    }

    let has_more = rows.len() > params.page_size;
    rows.truncate(params.page_size);
    let next_cursor = if has_more {
        rows.last()
            .map(|entry| {
                let (position, kind, id) = entry_key(entry);
                serde_json::to_string(&TimelineCursor {
                    thread_id: params.thread_id,
                    position,
                    kind,
                    id: id.to_string(),
                })
                .map_err(thread_history_error)
            })
            .transpose()?
    } else {
        None
    };

    let (page_start, page_start_kind, page_start_id) =
        rows.last().map(entry_key).unwrap_or_default();
    let mut active_realtime_session_at_page_start = None;
    for segment in lineage.segments().iter().rev() {
        let upper = page_start.min(
            segment
                .end_ordinal()
                .map(|ordinal| ordinal.saturating_sub(1))
                .unwrap_or(u64::MAX),
        );
        if upper < segment.start_ordinal() {
            continue;
        }
        let boundary = sqlx::query_as::<_, (String, String)>(
            r#"
SELECT item_type, json_extract(item_json, '$.realtime_session_id')
FROM thread_realtime_items
WHERE thread_id = ?
  AND rollout_ordinal >= ?
  AND rollout_ordinal <= ?
  AND (rollout_ordinal, 2, item_id) < (?, ?, ?)
  AND item_type IN ('realtime_session_started', 'realtime_session_closed')
ORDER BY rollout_ordinal DESC
LIMIT 1
            "#,
        )
        .bind(segment.rollout_id().to_string())
        .bind(sqlite_integer(segment.start_ordinal(), "rollout ordinal")?)
        .bind(sqlite_integer(upper, "rollout ordinal")?)
        .bind(sqlite_integer(page_start, "rollout ordinal")?)
        .bind(i64::from(page_start_kind))
        .bind(page_start_id)
        .fetch_optional(pool)
        .await
        .map_err(thread_history_error)?;
        if let Some((item_type, session_id)) = boundary {
            if item_type == "realtime_session_started" {
                active_realtime_session_at_page_start = Some(session_id);
            }
            break;
        }
    }

    rows.reverse();
    Ok(TimelinePage {
        items: rows,
        next_cursor,
        active_realtime_session_at_page_start,
    })
}
