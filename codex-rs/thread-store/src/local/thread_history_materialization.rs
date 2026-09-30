use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;

use chrono::DateTime;
use codex_app_server_protocol::ThreadHistoryChangeSet;
use codex_app_server_protocol::project_rollout_line;
use codex_protocol::ThreadId;
use codex_rollout::RolloutItem;
use tracing::warn;

use super::LocalThreadStore;
use super::thread_history::ProjectedRolloutLine;
use super::thread_history::RolloutProjectionStep;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

const SQLITE_PROJECTION_METRIC: &str = "codex.thread_history.sqlite_projection";
const SQLITE_PROJECTION_ANOMALY_METRIC: &str = "codex.thread_history.sqlite_projection.anomaly";

pub(super) async fn materialize_to_sqlite(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    rollout_path: &Path,
) -> ThreadStoreResult<()> {
    if store.state_db.is_none() {
        return Ok(());
    }
    let result = materialize_to_sqlite_with_state_db(store, thread_id, rollout_path).await;
    record_projection_outcome(&result);
    result
}

async fn materialize_to_sqlite_with_state_db(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    rollout_path: &Path,
) -> ThreadStoreResult<()> {
    let projection_state = super::thread_history::projection_state(store, thread_id).await?;
    let start_offset = projection_state
        .as_ref()
        .map_or(0, |state| state.next_byte_offset);
    if projection_state.is_none()
        && codex_rollout::existing_rollout_path(rollout_path)
            .await
            .is_none()
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
    let path = rollout_path.to_path_buf();
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        let _entered = span.enter();
        read_projection_steps_sync(
            &path,
            start_offset,
            expected_ordinal,
            thread_id,
            subagent_history_start_ordinal,
        )
    })
    .await
    .map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to join rollout projection read: {err}"),
    })?
}

/// Read and project the complete durable prefix in one blocking task.
fn read_projection_steps_sync(
    rollout_path: &Path,
    start_offset: u64,
    expected_ordinal: u64,
    thread_id: ThreadId,
    subagent_history_start_ordinal: Option<u64>,
) -> ThreadStoreResult<(Vec<RolloutProjectionStep>, u64)> {
    let mut file = match codex_rollout::open_rollout_seekable_reader(rollout_path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound && start_offset == 0 => {
            return Ok((Vec::new(), 0));
        }
        Err(err) => return Err(thread_store_io_error(err)),
    };
    let file_end_offset = file.metadata().map_err(thread_store_io_error)?.len();
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
    file.seek(SeekFrom::Start(start_offset))
        .map_err(thread_store_io_error)?;
    file.read_exact(bytes.as_mut_slice())
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
    let mut line_start_offset = start_offset;
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
            next_offset = line_end_offset;
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
                    "skipping malformed rollout line during projection"
                );
                record_projection_anomaly(ProjectionAnomaly::MalformedJson);
                next_offset = line_end_offset;
                line_start_offset = line_end_offset;
                continue;
            }
        };
        let raw_ordinal = value.get("ordinal").and_then(serde_json::Value::as_u64);
        let line = match codex_rollout::decode_rollout_line(value) {
            Ok(line) => line,
            Err(err) => {
                warn!(
                    thread_id = %thread_id,
                    rollout_path = %rollout_path.display(),
                    line_start_byte_offset = line_start_offset,
                    line_end_byte_offset = line_end_offset,
                    expected_ordinal = next_ordinal,
                    line_ordinal = ?raw_ordinal,
                    error = %err,
                    "skipping unknown rollout line during projection"
                );
                record_projection_anomaly(ProjectionAnomaly::UnknownLine);
                // Unknown records only advance the byte checkpoint. A writer that could not
                // decode this line may have reused its ordinal, so let the next decoded line
                // resolve reuse versus a gap.
                next_offset = line_end_offset;
                line_start_offset = line_end_offset;
                continue;
            }
        };
        let ordinal = match line.ordinal {
            Some(ordinal) => ordinal,
            None => {
                warn!(
                    thread_id = %thread_id,
                    rollout_path = %rollout_path.display(),
                    line_start_byte_offset = line_start_offset,
                    line_end_byte_offset = line_end_offset,
                    expected_ordinal = next_ordinal,
                    "skipping paginated rollout line without an ordinal"
                );
                record_projection_anomaly(ProjectionAnomaly::MissingOrdinal);
                next_offset = line_end_offset;
                line_start_offset = line_end_offset;
                continue;
            }
        };
        if ordinal < next_ordinal {
            warn!(
                thread_id = %thread_id,
                rollout_path = %rollout_path.display(),
                line_start_byte_offset = line_start_offset,
                line_end_byte_offset = line_end_offset,
                expected_ordinal = next_ordinal,
                line_ordinal = ordinal,
                "skipping duplicate or regressed rollout ordinal during projection"
            );
            record_projection_anomaly(ProjectionAnomaly::DuplicateOrRegressedOrdinal);
            next_offset = line_end_offset;
            line_start_offset = line_end_offset;
            continue;
        }
        let is_inherited_subagent_history =
            subagent_history_start_ordinal.is_some_and(|start| ordinal < start);
        let changes = if is_inherited_subagent_history {
            ThreadHistoryChangeSet::default()
        } else {
            project_rollout_line(&line)
        };
        let fallback_created_at_ms = if changes
            .changed_items
            .iter()
            .any(|item| item.started_at_ms.is_none())
            || (!is_inherited_subagent_history
                && matches!(&line.item, RolloutItem::RealtimeItem(_)))
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
                        "skipping rollout line with invalid timestamp during projection"
                    );
                    record_projection_anomaly(ProjectionAnomaly::InvalidTimestamp);
                    let end_ordinal_exclusive =
                        ordinal
                            .checked_add(1)
                            .ok_or_else(|| ThreadStoreError::Internal {
                                message: "rollout ordinal exceeds SQLite integer range".to_string(),
                            })?;
                    projections.push(RolloutProjectionStep::SkippedOrdinalRange {
                        start_ordinal: next_ordinal,
                        end_ordinal_exclusive,
                    });
                    next_ordinal = end_ordinal_exclusive;
                    next_offset = line_end_offset;
                    line_start_offset = line_end_offset;
                    continue;
                }
            }
        } else {
            None
        };
        if ordinal > next_ordinal {
            warn!(
                thread_id = %thread_id,
                rollout_path = %rollout_path.display(),
                line_start_byte_offset = line_start_offset,
                line_end_byte_offset = line_end_offset,
                expected_ordinal = next_ordinal,
                line_ordinal = ordinal,
                skipped_ordinal_start = next_ordinal,
                skipped_ordinal_end_exclusive = ordinal,
                "skipping missing rollout ordinal range during projection"
            );
            record_projection_anomaly(ProjectionAnomaly::ForwardOrdinalGap);
            projections.push(RolloutProjectionStep::SkippedOrdinalRange {
                start_ordinal: next_ordinal,
                end_ordinal_exclusive: ordinal,
            });
        }
        let next_line_ordinal =
            ordinal
                .checked_add(1)
                .ok_or_else(|| ThreadStoreError::Internal {
                    message: "rollout ordinal exceeds SQLite integer range".to_string(),
                })?;
        projections.push(RolloutProjectionStep::Line(Box::new(
            ProjectedRolloutLine {
                ordinal,
                start_byte_offset: line_start_offset,
                end_byte_offset: line_end_offset,
                fallback_created_at_ms,
                changes,
                realtime_item: match line.item {
                    RolloutItem::RealtimeItem(item) if !is_inherited_subagent_history => Some(item),
                    _ => None,
                },
            },
        )));
        next_ordinal = next_line_ordinal;
        next_offset = line_end_offset;
        line_start_offset = line_end_offset;
    }
    Ok((projections, next_offset))
}

#[derive(Clone, Copy)]
enum ProjectionAnomaly {
    MalformedJson,
    UnknownLine,
    MissingOrdinal,
    DuplicateOrRegressedOrdinal,
    ForwardOrdinalGap,
    InvalidTimestamp,
}

impl ProjectionAnomaly {
    fn tag(self) -> &'static str {
        match self {
            Self::MalformedJson => "malformed_json",
            Self::UnknownLine => "unknown_line",
            Self::MissingOrdinal => "missing_ordinal",
            Self::DuplicateOrRegressedOrdinal => "duplicate_or_regressed_ordinal",
            Self::ForwardOrdinalGap => "forward_ordinal_gap",
            Self::InvalidTimestamp => "invalid_timestamp",
        }
    }
}

fn record_projection_outcome(result: &ThreadStoreResult<()>) {
    let Some(metrics) = codex_otel::global() else {
        return;
    };
    let outcome = if result.is_ok() { "success" } else { "error" };
    let _ = metrics.counter(
        SQLITE_PROJECTION_METRIC,
        /*inc*/ 1,
        &[("outcome", outcome)],
    );
}

fn record_projection_anomaly(anomaly: ProjectionAnomaly) {
    let Some(metrics) = codex_otel::global() else {
        return;
    };
    let _ = metrics.counter(
        SQLITE_PROJECTION_ANOMALY_METRIC,
        /*inc*/ 1,
        &[("kind", anomaly.tag())],
    );
}

fn thread_store_io_error(err: std::io::Error) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: err.to_string(),
    }
}

#[cfg(test)]
#[path = "thread_history_materialization_tests.rs"]
mod tests;
