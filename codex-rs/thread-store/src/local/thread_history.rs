use codex_app_server_protocol::ThreadHistoryChangeSet;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::TurnStatus;
use codex_protocol::ThreadId;
use codex_protocol::models::MessagePhase;

use super::LocalThreadStore;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

mod read;

pub(super) use read::list_items;
pub(super) use read::list_turns;

pub(super) async fn next_rollout_byte_offset(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<u64> {
    let db_path = codex_state::thread_history_db_path(store.config.sqlite_home.as_path());
    if !tokio::fs::try_exists(db_path.as_path())
        .await
        .map_err(thread_history_error)?
    {
        return Ok(0);
    }

    let pool = store.thread_history_db().await?;
    let offset = sqlx::query_scalar::<_, i64>(
        "SELECT next_rollout_byte_offset FROM thread_history_projection_state WHERE thread_id = ?",
    )
    .bind(thread_id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(thread_history_error)?
    .unwrap_or(0);
    u64::try_from(offset).map_err(|_| ThreadStoreError::Internal {
        message: format!("thread history projection for {thread_id} has a negative byte offset"),
    })
}

pub(super) async fn apply_projection(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    start_offset: u64,
    next_offset: u64,
    projections: Vec<(Option<u64>, i64, ThreadHistoryChangeSet)>,
) -> ThreadStoreResult<()> {
    let pool = store.thread_history_db().await?;
    // Write the projected rows and advance the JSONL offset and ordinal in one transaction. If
    // SQLite fails, it stays behind the durable rollout instead of claiming data it did not
    // materialize.
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(thread_history_error)?;
    let thread_id = thread_id.to_string();
    let projection_state = sqlx::query_as::<_, (i64, i64)>(
        r#"
SELECT next_rollout_byte_offset, next_rollout_ordinal
FROM thread_history_projection_state
WHERE thread_id = ?
        "#,
    )
    .bind(thread_id.as_str())
    .fetch_optional(&mut *transaction)
    .await
    .map_err(thread_history_error)?;
    let (expected_offset, mut next_ordinal) = projection_state.unwrap_or((0, 0));
    let start_offset = sqlite_integer(start_offset, "rollout byte offset")?;
    if expected_offset != start_offset {
        return Err(ThreadStoreError::Internal {
            message: format!("thread history projection for {thread_id} is behind durable rollout"),
        });
    }

    for (ordinal, created_at_ms, changes) in projections {
        let ordinal = ordinal
            .ok_or_else(|| ThreadStoreError::Internal {
                message: format!("paginated rollout line for {thread_id} is missing an ordinal"),
            })
            .and_then(|ordinal| sqlite_integer(ordinal, "rollout ordinal"))?;
        if ordinal != next_ordinal {
            return Err(ThreadStoreError::Internal {
                message: format!(
                    "thread history projection for {thread_id} expected ordinal {next_ordinal}, got {ordinal}"
                ),
            });
        }
        apply_change_set(
            &mut transaction,
            thread_id.as_str(),
            ordinal,
            created_at_ms,
            changes,
        )
        .await?;
        next_ordinal = next_ordinal
            .checked_add(1)
            .ok_or_else(|| ThreadStoreError::Internal {
                message: "rollout ordinal exceeds SQLite integer range".to_string(),
            })?;
    }

    sqlx::query(
        r#"
INSERT INTO thread_history_projection_state (
    thread_id,
    next_rollout_byte_offset,
    next_rollout_ordinal
) VALUES (?, ?, ?)
ON CONFLICT(thread_id) DO UPDATE SET
    next_rollout_byte_offset = excluded.next_rollout_byte_offset,
    next_rollout_ordinal = excluded.next_rollout_ordinal
        "#,
    )
    .bind(thread_id.as_str())
    .bind(sqlite_integer(next_offset, "rollout byte offset")?)
    .bind(next_ordinal)
    .execute(&mut *transaction)
    .await
    .map_err(thread_history_error)?;
    transaction.commit().await.map_err(thread_history_error)
}

pub(super) async fn delete_thread(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<()> {
    let db_path = codex_state::thread_history_db_path(store.config.sqlite_home.as_path());
    if !tokio::fs::try_exists(db_path.as_path())
        .await
        .map_err(thread_history_delete_error)?
    {
        return Ok(());
    }

    let pool = store.thread_history_db().await?;
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(thread_history_delete_error)?;
    let thread_id = thread_id.to_string();
    sqlx::query("DELETE FROM thread_items WHERE thread_id = ?")
        .bind(thread_id.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(thread_history_delete_error)?;
    sqlx::query("DELETE FROM thread_turns WHERE thread_id = ?")
        .bind(thread_id.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(thread_history_delete_error)?;
    sqlx::query("DELETE FROM thread_history_projection_state WHERE thread_id = ?")
        .bind(thread_id.as_str())
        .execute(&mut *transaction)
        .await
        .map_err(thread_history_delete_error)?;
    transaction
        .commit()
        .await
        .map_err(thread_history_delete_error)
}

async fn apply_change_set(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    thread_id: &str,
    rollout_ordinal: i64,
    created_at_ms: i64,
    changes: ThreadHistoryChangeSet,
) -> ThreadStoreResult<()> {
    for turn in changes.changed_turns {
        let turn_id = turn.turn_id;
        let error_json = turn
            .error
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(thread_history_error)?;
        // The same turn can appear again as it moves from started to completed. Update its latest
        // status, error, and timestamps, but keep the rollout ordinal from the first record that
        // created it.
        sqlx::query(
            r#"
INSERT INTO thread_turns (
    thread_id,
    turn_id,
    rollout_ordinal,
    status,
    error_json,
    started_at,
    completed_at,
    duration_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(thread_id, turn_id) DO UPDATE SET
    status = excluded.status,
    error_json = excluded.error_json,
    started_at = excluded.started_at,
    completed_at = excluded.completed_at,
    duration_ms = excluded.duration_ms
            "#,
        )
        .bind(thread_id)
        .bind(turn_id.as_str())
        .bind(rollout_ordinal)
        .bind(turn_status(&turn.status))
        .bind(error_json)
        .bind(turn.started_at)
        .bind(turn.completed_at)
        .bind(turn.duration_ms)
        .execute(&mut **transaction)
        .await
        .map_err(thread_history_error)?;

        // Review turns can persist completed items before their turn lifecycle record. Fill the
        // summary IDs from those older item rows when the turn row finally arrives.
        sqlx::query(
            r#"
UPDATE thread_turns
SET
    first_user_item_id = COALESCE(
        first_user_item_id,
        (
            SELECT item_id
            FROM thread_items
            WHERE thread_id = ?
              AND turn_id = ?
              AND json_extract(item_json, '$.type') = 'userMessage'
            ORDER BY rollout_ordinal
            LIMIT 1
        )
    ),
    final_agent_item_id = COALESCE(
        (
            SELECT item_id
            FROM thread_items
            WHERE thread_id = ?
              AND turn_id = ?
              AND json_extract(item_json, '$.type') = 'agentMessage'
              AND json_extract(item_json, '$.phase') = 'final_answer'
            ORDER BY rollout_ordinal DESC
            LIMIT 1
        ),
        CASE
            WHEN status IN ('completed', 'interrupted', 'failed') THEN (
                SELECT item_id
                FROM thread_items
                WHERE thread_id = ?
                  AND turn_id = ?
                  AND json_extract(item_json, '$.type') = 'agentMessage'
                  AND json_extract(item_json, '$.phase') IS NULL
                ORDER BY rollout_ordinal DESC
                LIMIT 1
            )
        END,
        final_agent_item_id
    )
WHERE thread_id = ? AND turn_id = ?
            "#,
        )
        .bind(thread_id)
        .bind(turn_id.as_str())
        .bind(thread_id)
        .bind(turn_id.as_str())
        .bind(thread_id)
        .bind(turn_id.as_str())
        .bind(thread_id)
        .bind(turn_id.as_str())
        .execute(&mut **transaction)
        .await
        .map_err(thread_history_error)?;
    }

    for item in changes.changed_items {
        let item_id = item.item.id().to_string();
        let item_json = serde_json::to_string(&item.item).map_err(thread_history_error)?;
        // The same item can appear again with a newer snapshot. Replace its JSON, but keep the
        // ordinal and creation timestamp from the first record so item ordering and age stay
        // stable.
        sqlx::query(
            r#"
INSERT INTO thread_items (
    thread_id,
    turn_id,
    item_id,
    rollout_ordinal,
    created_at_ms,
    item_json
) VALUES (?, ?, ?, ?, ?, ?)
ON CONFLICT(thread_id, turn_id, item_id) DO UPDATE SET
    item_json = excluded.item_json
            "#,
        )
        .bind(thread_id)
        .bind(item.turn_id.as_str())
        .bind(item_id.as_str())
        .bind(rollout_ordinal)
        .bind(created_at_ms)
        .bind(item_json)
        .execute(&mut **transaction)
        .await
        .map_err(thread_history_error)?;

        // Keep summary item IDs on the turn row so reads do not need to scan every item in the
        // turn.
        match item.item {
            ThreadItem::UserMessage { .. } => {
                sqlx::query(
                    r#"
UPDATE thread_turns
SET first_user_item_id = COALESCE(first_user_item_id, ?)
WHERE thread_id = ? AND turn_id = ?
                    "#,
                )
                .bind(item_id.as_str())
                .bind(thread_id)
                .bind(item.turn_id.as_str())
                .execute(&mut **transaction)
                .await
                .map_err(thread_history_error)?;
            }
            ThreadItem::AgentMessage {
                phase: Some(MessagePhase::FinalAnswer),
                ..
            } => {
                sqlx::query(
                    r#"
UPDATE thread_turns
SET final_agent_item_id = ?
WHERE thread_id = ? AND turn_id = ?
                    "#,
                )
                .bind(item_id.as_str())
                .bind(thread_id)
                .bind(item.turn_id.as_str())
                .execute(&mut **transaction)
                .await
                .map_err(thread_history_error)?;
            }
            ThreadItem::AgentMessage {
                phase: Some(MessagePhase::Commentary) | None,
                ..
            }
            | ThreadItem::HookPrompt { .. }
            | ThreadItem::Plan { .. }
            | ThreadItem::Reasoning { .. }
            | ThreadItem::CommandExecution { .. }
            | ThreadItem::FileChange { .. }
            | ThreadItem::McpToolCall { .. }
            | ThreadItem::DynamicToolCall { .. }
            | ThreadItem::CollabAgentToolCall { .. }
            | ThreadItem::SubAgentActivity { .. }
            | ThreadItem::WebSearch(_)
            | ThreadItem::ImageView { .. }
            | ThreadItem::Sleep(_)
            | ThreadItem::ImageGeneration(_)
            | ThreadItem::EnteredReviewMode { .. }
            | ThreadItem::ExitedReviewMode { .. }
            | ThreadItem::ContextCompaction { .. } => {}
        }
    }
    Ok(())
}

fn turn_status(status: &TurnStatus) -> &'static str {
    match status {
        TurnStatus::Completed => "completed",
        TurnStatus::Interrupted => "interrupted",
        TurnStatus::Failed => "failed",
        TurnStatus::InProgress => "inProgress",
    }
}

fn sqlite_integer(value: u64, field: &str) -> ThreadStoreResult<i64> {
    i64::try_from(value).map_err(|_| ThreadStoreError::Internal {
        message: format!("{field} exceeds SQLite integer range"),
    })
}

fn thread_history_error(err: impl std::fmt::Display) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: format!("failed to access thread history: {err}"),
    }
}

impl From<sqlx::Error> for ThreadStoreError {
    fn from(err: sqlx::Error) -> Self {
        thread_history_error(err)
    }
}

fn thread_history_delete_error(err: impl std::fmt::Display) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: format!("failed to delete thread history: {err}"),
    }
}
