//! Pipes bounded rollout batches into short SQLite transactions, committing each checkpoint.

use std::path::Path;

use codex_protocol::ThreadId;

use super::super::LocalThreadStore;
use super::super::thread_history::RolloutProjectionStep;
use super::super::thread_history::apply_projection;
use super::read_projection_batches;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

const QUEUED_BATCHES: usize = 2;

pub(super) enum ProjectionMessage {
    Batch {
        projections: Vec<RolloutProjectionStep>,
        next_offset: u64,
    },
    Complete,
    Failed(ThreadStoreError),
}

pub(super) async fn read_and_apply(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    rollout_path: &Path,
    start_offset: u64,
    expected_ordinal: u64,
    initial_ordinal: u64,
    subagent_history_start_ordinal: Option<u64>,
) -> ThreadStoreResult<()> {
    // Two queued batches, one being produced and one being consumed. Each is limited by
    // source bytes and step count; an oversized individual record remains indivisible.
    let (sender, receiver) = tokio::sync::mpsc::channel(QUEUED_BATCHES);
    let (stop, keep_running) = tokio::sync::oneshot::channel::<()>();
    let path = rollout_path.to_path_buf();
    let span = tracing::Span::current();
    let worker = tokio::task::spawn_blocking(move || {
        let _entered = span.enter();
        let read = read_projection_batches(
            &path,
            start_offset,
            expected_ordinal,
            thread_id,
            subagent_history_start_ordinal,
            || stop.is_closed(),
            |projections, next_offset| {
                sender
                    .blocking_send(ProjectionMessage::Batch {
                        projections,
                        next_offset,
                    })
                    .map_err(|_| ThreadStoreError::Internal {
                        message: "projection consumer stopped".to_string(),
                    })
            },
        );
        let completion = match read {
            Ok(()) => ProjectionMessage::Complete,
            Err(error) => ProjectionMessage::Failed(error),
        };
        let _ = sender.blocking_send(completion);
    });
    let result = apply_batches(store, thread_id, start_offset, initial_ordinal, receiver).await;
    // apply_batches drops the receiver on error, unblocking any pending producer send.
    drop(keep_running);
    worker.await.map_err(|err| ThreadStoreError::Internal {
        message: format!("projection producer failed: {err}"),
    })?;
    result
}

pub(super) async fn apply_batches(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    mut start_offset: u64,
    initial_ordinal: u64,
    mut receiver: tokio::sync::mpsc::Receiver<ProjectionMessage>,
) -> ThreadStoreResult<()> {
    // Receive before opening the transaction so a slow reader cannot hold the SQLite writer.
    while let Some(message) = receiver.recv().await {
        match message {
            ProjectionMessage::Batch {
                projections,
                next_offset,
            } => {
                apply_projection(
                    store,
                    thread_id,
                    start_offset,
                    next_offset,
                    initial_ordinal,
                    projections,
                )
                .await?;
                start_offset = next_offset;
            }
            ProjectionMessage::Complete => return Ok(()),
            ProjectionMessage::Failed(error) => return Err(error),
        }
    }
    Err(ThreadStoreError::Internal {
        message: "projection producer stopped before successful completion".to_string(),
    })
}
