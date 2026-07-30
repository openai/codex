use std::io::SeekFrom;
use std::path::Path;

use chrono::DateTime;
use codex_app_server_protocol::ThreadHistoryChangeSet;
use codex_app_server_protocol::project_rollout_line;
use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutLine;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncSeekExt;
use tracing::warn;

use super::LocalThreadStore;
use super::thread_history::ProjectedRolloutLine;
use super::thread_history::RolloutProjectionStep;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) async fn materialize_to_sqlite(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    rollout_path: &Path,
) -> ThreadStoreResult<()> {
    if store.state_db.is_none() {
        return Ok(());
    }
    let projection_state = super::thread_history::projection_state(store, thread_id).await?;
    let start_offset = projection_state
        .as_ref()
        .map_or(0, |state| state.next_byte_offset);
    if projection_state.is_none()
        && !tokio::fs::try_exists(rollout_path)
            .await
            .map_err(thread_store_io_error)?
    {
        return Ok(());
    }
    let session_meta = codex_rollout::read_session_meta_line(rollout_path)
        .await
        .map_err(thread_store_io_error)?
        .meta;
    let initial_ordinal = session_meta
        .history_base
        .map_or(0, |base| base.end_ordinal_exclusive);
    let subagent_history_start_ordinal = session_meta.subagent_history_start_ordinal;
    let expected_ordinal = projection_state
        .as_ref()
        .map_or(initial_ordinal, |state| state.next_ordinal);
    let (projections, next_offset) = read_projection_steps(
        rollout_path,
        start_offset,
        expected_ordinal,
        thread_id,
        subagent_history_start_ordinal,
    )
    .await?;
    // Empty valid records can still consume bytes through blank complete lines.
    if projections.is_empty() && start_offset == next_offset {
        return Ok(());
    }
    super::thread_history::apply_projection(
        store,
        thread_id,
        start_offset,
        next_offset,
        initial_ordinal,
        projections,
    )
    .await
}

async fn read_projection_steps(
    rollout_path: &Path,
    start_offset: u64,
    expected_ordinal: u64,
    thread_id: ThreadId,
    subagent_history_start_ordinal: Option<u64>,
) -> ThreadStoreResult<(Vec<RolloutProjectionStep>, u64)> {
    let file_end_offset = match tokio::fs::metadata(rollout_path).await {
        Ok(metadata) => metadata.len(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound && start_offset == 0 => {
            return Ok((Vec::new(), 0));
        }
        Err(err) => return Err(thread_store_io_error(err)),
    };
    let byte_count =
        file_end_offset
            .checked_sub(start_offset)
            .ok_or_else(|| ThreadStoreError::Internal {
                message: "durable rollout shrank before projection".to_string(),
            })?;
    let byte_count = usize::try_from(byte_count).map_err(|_| ThreadStoreError::Internal {
        message: "durable rollout append exceeds addressable memory".to_string(),
    })?;
    let mut bytes = vec![0; byte_count];
    let mut file = tokio::fs::File::open(rollout_path)
        .await
        .map_err(thread_store_io_error)?;
    file.seek(SeekFrom::Start(start_offset))
        .await
        .map_err(thread_store_io_error)?;
    file.read_exact(bytes.as_mut_slice())
        .await
        .map_err(thread_store_io_error)?;
    // Only project the newline-terminated prefix; leave a trailing partial record for the next
    // pass.
    let complete_byte_count = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    let mut projections = Vec::new();
    let mut next_ordinal = expected_ordinal;
    let mut next_offset = start_offset;
    let mut pending_rejected_line_count = 0;
    let mut line_start_offset = start_offset;
    // Keep rejected lines pending until a later valid ordinal proves whether they consumed history.
    // This lets a same-ordinal retry replace a failed write without advancing only one checkpoint.
    for line_bytes in bytes[..complete_byte_count].split_inclusive(|byte| *byte == b'\n') {
        let line_end_offset = line_start_offset
            .checked_add(u64::try_from(line_bytes.len()).map_err(|_| {
                ThreadStoreError::Internal {
                    message: "durable rollout byte offset overflow".to_string(),
                }
            })?)
            .ok_or_else(|| ThreadStoreError::Internal {
                message: "durable rollout byte offset overflow".to_string(),
            })?;
        if line_bytes.iter().all(u8::is_ascii_whitespace) {
            if pending_rejected_line_count == 0 {
                next_offset = line_end_offset;
            }
            line_start_offset = line_end_offset;
            continue;
        }
        let value = match serde_json::from_slice::<serde_json::Value>(line_bytes) {
            Ok(value) => value,
            Err(err) => {
                warn!(
                    thread_id = %thread_id,
                    rollout_path = %rollout_path.display(),
                    line_start_byte_offset = line_start_offset,
                    line_end_byte_offset = line_end_offset,
                    expected_ordinal = next_ordinal,
                    error = %err,
                    "deferring rejected rollout line until a later ordinal resolves it"
                );
                pending_rejected_line_count += 1;
                line_start_offset = line_end_offset;
                continue;
            }
        };
        let value_ordinal = value.get("ordinal").and_then(serde_json::Value::as_u64);
        let line = match serde_json::from_value::<RolloutLine>(value) {
            Ok(line) => Some(line),
            Err(err) => {
                warn!(
                    thread_id = %thread_id,
                    rollout_path = %rollout_path.display(),
                    line_start_byte_offset = line_start_offset,
                    line_end_byte_offset = line_end_offset,
                    expected_ordinal = next_ordinal,
                    line_ordinal = ?value_ordinal,
                    error = %err,
                    "deferring unknown rollout line until a later ordinal resolves it"
                );
                None
            }
        };
        let ordinal = match line
            .as_ref()
            .and_then(|line| line.ordinal)
            .or(value_ordinal)
        {
            Some(ordinal) => ordinal,
            None if line.is_none() => {
                pending_rejected_line_count += 1;
                line_start_offset = line_end_offset;
                continue;
            }
            None => {
                return Err(ThreadStoreError::Internal {
                    message: format!(
                        "paginated rollout line for {thread_id} is missing an ordinal"
                    ),
                });
            }
        };
        if ordinal < next_ordinal {
            return Err(ThreadStoreError::Internal {
                message: format!(
                    "thread history projection for {thread_id} expected ordinal {next_ordinal}, got {ordinal}"
                ),
            });
        }
        let Some(line) = line else {
            pending_rejected_line_count += 1;
            line_start_offset = line_end_offset;
            continue;
        };
        let skipped_ordinal_count = ordinal - next_ordinal;
        if skipped_ordinal_count > pending_rejected_line_count {
            return Err(ThreadStoreError::Internal {
                message: format!(
                    "thread history projection for {thread_id} expected ordinal {next_ordinal}, got {ordinal}; {pending_rejected_line_count} rejected rollout lines cannot cover that gap"
                ),
            });
        }
        let changes = if subagent_history_start_ordinal.is_some_and(|start| ordinal < start) {
            ThreadHistoryChangeSet::default()
        } else {
            project_rollout_line(&line)
        };
        let fallback_created_at_ms = if changes
            .changed_items
            .iter()
            .any(|item| item.started_at_ms.is_none())
        {
            match DateTime::parse_from_rfc3339(line.timestamp.as_str()) {
                Ok(timestamp) => Some(timestamp.timestamp_millis()),
                Err(err) => {
                    warn!(
                        thread_id = %thread_id,
                        rollout_path = %rollout_path.display(),
                        line_start_byte_offset = line_start_offset,
                        line_end_byte_offset = line_end_offset,
                        expected_ordinal = next_ordinal,
                        line_ordinal = ordinal,
                        error = %err,
                        "deferring rollout line with invalid timestamp until a later ordinal resolves it"
                    );
                    pending_rejected_line_count += 1;
                    line_start_offset = line_end_offset;
                    continue;
                }
            }
        } else {
            None
        };
        if skipped_ordinal_count > 0 {
            warn!(
                thread_id = %thread_id,
                rollout_path = %rollout_path.display(),
                line_start_byte_offset = line_start_offset,
                line_end_byte_offset = line_end_offset,
                expected_ordinal = next_ordinal,
                line_ordinal = ordinal,
                skipped_ordinal_start = next_ordinal,
                skipped_ordinal_end_exclusive = ordinal,
                "skipping rollout ordinal range after rejected lines"
            );
            projections.push(RolloutProjectionStep::SkippedOrdinalRange {
                start_ordinal: next_ordinal,
                end_ordinal_exclusive: ordinal,
            });
        }
        pending_rejected_line_count = 0;
        let next_line_ordinal =
            ordinal
                .checked_add(1)
                .ok_or_else(|| ThreadStoreError::Internal {
                    message: "rollout ordinal exceeds SQLite integer range".to_string(),
                })?;
        projections.push(RolloutProjectionStep::Line(ProjectedRolloutLine {
            ordinal,
            start_byte_offset: line_start_offset,
            end_byte_offset: line_end_offset,
            fallback_created_at_ms,
            changes,
        }));
        next_ordinal = next_line_ordinal;
        next_offset = line_end_offset;
        line_start_offset = line_end_offset;
    }
    Ok((projections, next_offset))
}

fn thread_store_io_error(err: std::io::Error) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: err.to_string(),
    }
}

#[cfg(test)]
#[path = "thread_history_materialization_tests.rs"]
mod tests;
