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
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) async fn materialize_to_sqlite(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    rollout_path: &Path,
) -> ThreadStoreResult<()> {
    let start_offset = super::thread_history::next_rollout_byte_offset(store, thread_id).await?;
    let (lines, next_offset) = read_complete_rollout_lines(rollout_path, start_offset).await?;
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
        .map(|line| {
            let created_at_ms = DateTime::parse_from_rfc3339(line.timestamp.as_str())
                .map(|timestamp| timestamp.timestamp_millis())
                .map_err(thread_history_error)?;
            let changes = if line.ordinal.is_some_and(|ordinal| {
                subagent_history_start_ordinal.is_some_and(|start| ordinal < start)
            }) {
                ThreadHistoryChangeSet::default()
            } else {
                project_rollout_line(line)
            };
            Ok((line.ordinal, created_at_ms, changes))
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
) -> ThreadStoreResult<(Vec<RolloutLine>, u64)> {
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
    let text = std::str::from_utf8(&bytes[..complete_byte_count]).map_err(thread_history_error)?;
    let mut lines = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        match serde_json::from_str(line) {
            Ok(line) => lines.push(line),
            Err(err) => {
                // A failed append can leave a partial record behind. The rollout writer repairs
                // its newline before retrying, so skip rejected lines just like the canonical
                // rollout loader and keep projecting the valid retry that follows.
                warn!("skipping rejected rollout line while projecting {rollout_path:?}: {err}");
            }
        }
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
