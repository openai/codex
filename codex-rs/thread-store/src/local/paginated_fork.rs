use codex_protocol::protocol::HistoryPosition;
use std::sync::Arc;

use super::LocalThreadStore;
use super::live_writer;
use super::model_context;
use super::thread_history::find_source_turn;
use super::thread_history::find_visible_turn;
use crate::ForkBoundary;
use crate::PrepareForkParams;
use crate::PreparedFork;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) async fn prepare(
    store: &LocalThreadStore,
    params: PrepareForkParams,
) -> ThreadStoreResult<PreparedFork> {
    let PrepareForkParams {
        thread_id,
        boundary,
    } = params;
    let source_reservation = store.live_writer_locks.reserve_lifecycle(thread_id).await;
    // Keep the source reserved until persistence and lineage materialization finish, even if the
    // caller cancels fork preparation.
    let lineage_store = store.clone();
    let (lineage, source_reservation) = tokio::spawn(async move {
        match live_writer::persist_thread(&lineage_store, thread_id).await {
            Ok(()) | Err(ThreadStoreError::ThreadNotFound { .. }) => {}
            Err(err) => return Err(err),
        }
        let lineage = lineage_store
            .resolve_rollout_lineage_for_reference(thread_id)
            .await?;
        Ok::<_, ThreadStoreError>((lineage, source_reservation))
    })
    .await
    .map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to resolve fork lineage: {err}"),
    })??;
    let source_segment = lineage
        .segments()
        .last()
        .ok_or_else(|| ThreadStoreError::Internal {
            message: "fork lineage has no source segment".to_string(),
        })?;
    if !matches!(boundary, ForkBoundary::Latest) {
        for segment in lineage
            .segments()
            .iter()
            .take(lineage.segments().len().saturating_sub(1))
        {
            let _ancestor_writer_guard = store.live_writer_locks.lock(segment.thread_id()).await;
            super::thread_history_materialization::materialize_to_sqlite(
                store,
                segment.thread_id(),
                segment.rollout_path.as_path(),
            )
            .await?;
        }
    }
    let source_writer_guard = store.live_writer_locks.lock(thread_id).await;
    super::thread_history_materialization::materialize_to_sqlite(
        store,
        thread_id,
        source_segment.rollout_path.as_path(),
    )
    .await?;

    let latest_projection_state = super::thread_history::projection_state(store, thread_id)
        .await?
        .ok_or_else(|| ThreadStoreError::Internal {
            message: format!("missing projection state for paginated thread {thread_id}"),
        })?;
    let latest_position = HistoryPosition {
        thread_id,
        end_ordinal_exclusive: latest_projection_state.next_ordinal,
        end_byte_offset: latest_projection_state.next_byte_offset,
    };
    let pool = store.thread_history_db().await?;
    let position = match boundary {
        ForkBoundary::Latest => latest_position,
        ForkBoundary::ThroughTurn(turn_id) => {
            let row = find_visible_turn(pool, &lineage, turn_id.as_str()).await?;
            if row.status == "inProgress" {
                return Err(ThreadStoreError::InvalidRequest {
                    message: format!("lastTurnId '{turn_id}' identifies an in-progress turn"),
                });
            }
            let rollout_end_ordinal = row
                .rollout_end_ordinal
                .ok_or_else(|| missing_turn_position(turn_id.as_str()))?;
            let rollout_end_byte_offset = row
                .rollout_end_byte_offset
                .ok_or_else(|| missing_turn_position(turn_id.as_str()))?;
            HistoryPosition {
                thread_id: row.physical_thread_id,
                end_ordinal_exclusive: u64::try_from(rollout_end_ordinal)
                    .map_err(|_| invalid_turn_position(turn_id.as_str()))?
                    .checked_add(1)
                    .ok_or_else(|| invalid_turn_position(turn_id.as_str()))?,
                end_byte_offset: u64::try_from(rollout_end_byte_offset)
                    .map_err(|_| invalid_turn_position(turn_id.as_str()))?,
            }
        }
        ForkBoundary::BeforeTurn(turn_id) => {
            let row = find_source_turn(pool, &lineage, turn_id.as_str()).await?;
            if row.rollout_end_ordinal == Some(row.rollout_ordinal) {
                return Err(ThreadStoreError::InvalidRequest {
                    message: format!("turn {turn_id} does not have a persisted start boundary"),
                });
            }
            let rollout_byte_offset = row
                .rollout_byte_offset
                .ok_or_else(|| missing_turn_position(turn_id.as_str()))?;
            HistoryPosition {
                thread_id: row.physical_thread_id,
                end_ordinal_exclusive: u64::try_from(row.rollout_ordinal)
                    .map_err(|_| invalid_turn_position(turn_id.as_str()))?,
                end_byte_offset: u64::try_from(rollout_byte_offset)
                    .map_err(|_| invalid_turn_position(turn_id.as_str()))?,
            }
        }
    };
    let segment_index = lineage
        .segments()
        .iter()
        .position(|segment| segment.thread_id() == position.thread_id)
        .ok_or_else(|| ThreadStoreError::Internal {
            message: "fork position is outside the source lineage".to_string(),
        })?;
    if lineage.segments()[segment_index].end.is_some_and(|end| {
        position.end_ordinal_exclusive > end.end_ordinal_exclusive
            || position.end_byte_offset > end.end_byte_offset
    }) {
        return Err(ThreadStoreError::InvalidRequest {
            message: "fork boundary exceeds inherited source history".to_string(),
        });
    }
    let history_base =
        if position.end_ordinal_exclusive == lineage.segments()[segment_index].start_ordinal() {
            segment_index
                .checked_sub(1)
                .and_then(|index| lineage.segments()[index].end)
        } else {
            Some(position)
        };
    drop(source_writer_guard);
    let model_context = Arc::new(model_context::load_for_fork(lineage, history_base).await?);

    Ok(PreparedFork::new(
        thread_id,
        history_base,
        model_context,
        source_reservation,
    ))
}

fn missing_turn_position(turn_id: &str) -> ThreadStoreError {
    ThreadStoreError::InvalidRequest {
        message: format!("turn {turn_id} does not have persisted rollout positions"),
    }
}

fn invalid_turn_position(turn_id: &str) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: format!("invalid rollout position for turn {turn_id}"),
    }
}
