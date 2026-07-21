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
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

struct CompleteRolloutLine {
    line: RolloutLine,
    start_byte_offset: u64,
    end_byte_offset: u64,
}

pub(super) async fn materialize_to_sqlite(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    rollout_path: &Path,
) -> ThreadStoreResult<()> {
    let start_offset = super::thread_history::next_rollout_byte_offset(store, thread_id).await?;
    let (lines, next_offset) = read_complete_rollout_lines(rollout_path, start_offset).await?;
    // Empty valid records can still consume bytes through blank or rejected complete lines.
    if lines.is_empty() && start_offset == next_offset {
        return Ok(());
    }
    let subagent_history_start_ordinal = codex_rollout::read_session_meta_line(rollout_path)
        .await
        .map_err(thread_store_io_error)?
        .meta
        .subagent_history_start_ordinal;

    let projections = lines
        .iter()
        .map(|record| {
            let line = &record.line;
            let ordinal = line.ordinal.ok_or_else(|| ThreadStoreError::Internal {
                message: format!("paginated rollout line for {thread_id} is missing an ordinal"),
            })?;
            let created_at_ms = DateTime::parse_from_rfc3339(line.timestamp.as_str())
                .map(|timestamp| timestamp.timestamp_millis())
                .map_err(thread_history_error)?;
            let changes = if subagent_history_start_ordinal.is_some_and(|start| ordinal < start) {
                ThreadHistoryChangeSet::default()
            } else {
                project_rollout_line(line)
            };
            Ok(ProjectedRolloutLine {
                ordinal,
                start_byte_offset: record.start_byte_offset,
                end_byte_offset: record.end_byte_offset,
                created_at_ms,
                changes,
            })
        })
        .collect::<ThreadStoreResult<Vec<_>>>()?;
    super::thread_history::apply_projection(
        store,
        thread_id,
        start_offset,
        next_offset,
        projections,
    )
    .await
}

async fn read_complete_rollout_lines(
    rollout_path: &Path,
    start_offset: u64,
) -> ThreadStoreResult<(Vec<CompleteRolloutLine>, u64)> {
    let next_offset = match tokio::fs::metadata(rollout_path).await {
        Ok(metadata) => metadata.len(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound && start_offset == 0 => {
            return Ok((Vec::new(), 0));
        }
        Err(err) => return Err(thread_store_io_error(err)),
    };
    let byte_count =
        next_offset
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
    let next_offset = start_offset
        .checked_add(u64::try_from(complete_byte_count).map_err(|_| {
            ThreadStoreError::Internal {
                message: "durable rollout append exceeds addressable memory".to_string(),
            }
        })?)
        .ok_or_else(|| ThreadStoreError::Internal {
            message: "durable rollout byte offset overflow".to_string(),
        })?;
    let mut lines = Vec::new();
    let mut line_start_offset = start_offset;
    // Preserve each complete physical line's trailing newline so byte offsets advance through
    // every durable byte, including blank or rejected lines that do not project a row.
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
        // Blank physical lines consume bytes but are not rollout records.
        if !line_bytes.iter().all(u8::is_ascii_whitespace) {
            match serde_json::from_slice(line_bytes) {
                Ok(line) => lines.push(CompleteRolloutLine {
                    line,
                    start_byte_offset: line_start_offset,
                    end_byte_offset: line_end_offset,
                }),
                Err(err) => {
                    // A failed append can leave a partial record behind. The rollout writer
                    // repairs its newline before retrying, so skip rejected lines just like the
                    // canonical rollout loader and keep projecting the valid retry that follows.
                    warn!(
                        "skipping rejected rollout line while projecting {rollout_path:?}: {err}"
                    );
                }
            }
        }
        line_start_offset = line_end_offset;
    }
    Ok((lines, next_offset))
}

fn thread_history_error(err: impl std::fmt::Display) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: format!("failed to project thread history: {err}"),
    }
}

fn thread_store_io_error(err: std::io::Error) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: err.to_string(),
    }
}

#[cfg(test)]
#[path = "thread_history_materialization_tests.rs"]
mod tests;
