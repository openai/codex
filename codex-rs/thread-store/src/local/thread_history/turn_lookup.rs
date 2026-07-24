use codex_protocol::ThreadId;
use sqlx::Row;

use super::super::rollout_lineage::RolloutLineage;
use super::super::rollout_lineage::RolloutLineageSegment;
use super::sqlite_integer;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(in crate::local) struct TurnRow {
    pub physical_thread_id: ThreadId,
    pub rollout_ordinal: i64,
    pub rollout_byte_offset: Option<i64>,
    pub rollout_end_ordinal: Option<i64>,
    pub rollout_end_byte_offset: Option<i64>,
    pub status: String,
    pub first_user_item_id: Option<String>,
    pub final_agent_item_id: Option<String>,
}

pub(in crate::local) async fn find_source_turn(
    pool: &sqlx::SqlitePool,
    lineage: &RolloutLineage,
    turn_id: &str,
) -> ThreadStoreResult<TurnRow> {
    find_turn(pool, lineage.segments().iter(), turn_id).await
}

pub(in crate::local) async fn find_visible_turn(
    pool: &sqlx::SqlitePool,
    lineage: &RolloutLineage,
    turn_id: &str,
) -> ThreadStoreResult<TurnRow> {
    find_turn(pool, lineage.segments().iter().rev(), turn_id).await
}

async fn find_turn<'a>(
    pool: &sqlx::SqlitePool,
    segments: impl Iterator<Item = &'a RolloutLineageSegment>,
    turn_id: &str,
) -> ThreadStoreResult<TurnRow> {
    for segment in segments {
        if let Some(row) = query_turn_row(pool, segment, turn_id).await? {
            return Ok(row);
        }
    }
    Err(ThreadStoreError::InvalidRequest {
        message: format!("turn not found: {turn_id}"),
    })
}

async fn query_turn_row(
    pool: &sqlx::SqlitePool,
    segment: &RolloutLineageSegment,
    turn_id: &str,
) -> ThreadStoreResult<Option<TurnRow>> {
    let end_ordinal = segment
        .end_ordinal()
        .map(|ordinal| sqlite_integer(ordinal, "rollout ordinal"))
        .transpose()?;
    sqlx::query(
        r#"
SELECT
    rollout_ordinal,
    rollout_byte_offset,
    rollout_end_ordinal,
    rollout_end_byte_offset,
    status,
    first_user_item_id,
    final_agent_item_id
FROM thread_turns
WHERE thread_id = ?
  AND turn_id = ?
  AND rollout_ordinal >= ?
  AND (? IS NULL OR rollout_ordinal < ?)
        "#,
    )
    .bind(segment.thread_id().to_string())
    .bind(turn_id)
    .bind(sqlite_integer(segment.start_ordinal(), "rollout ordinal")?)
    .bind(end_ordinal)
    .bind(end_ordinal)
    .fetch_optional(pool)
    .await
    .map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to resolve logical turn: {err}"),
    })
    .map(|row| {
        row.map(|row| TurnRow {
            physical_thread_id: segment.thread_id(),
            rollout_ordinal: row.get("rollout_ordinal"),
            rollout_byte_offset: row.get("rollout_byte_offset"),
            rollout_end_ordinal: row.get("rollout_end_ordinal"),
            rollout_end_byte_offset: row.get("rollout_end_byte_offset"),
            status: row.get("status"),
            first_user_item_id: row.get("first_user_item_id"),
            final_agent_item_id: row.get("final_agent_item_id"),
        })
    })
}
