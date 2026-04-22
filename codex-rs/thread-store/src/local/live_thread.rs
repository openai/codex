use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_rollout::RolloutRecorder;
use codex_rollout::RolloutRecorderParams;
use codex_rollout::builder_from_items;
use codex_rollout::state_db;

use super::LocalThreadStore;
use super::create_thread;
use crate::AppendThreadItemsParams;
use crate::CreateThreadParams;
use crate::ReadThreadParams;
use crate::ResumeThreadParams;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) async fn create_thread(
    store: &LocalThreadStore,
    params: CreateThreadParams,
) -> ThreadStoreResult<()> {
    let thread_id = params.thread_id;
    let recorder = create_thread::create_thread(store, params).await?;
    store.insert_live_recorder(thread_id, recorder).await;
    Ok(())
}

pub(super) async fn resume_thread(
    store: &LocalThreadStore,
    params: ResumeThreadParams,
) -> ThreadStoreResult<()> {
    let (rollout_path, history) = match (params.rollout_path, params.history) {
        (Some(rollout_path), history) => (rollout_path, history),
        (None, history) => {
            let thread = super::read_thread::read_thread(
                store,
                ReadThreadParams {
                    thread_id: params.thread_id,
                    include_archived: params.include_archived,
                    include_history: history.is_none(),
                },
            )
            .await?;
            let rollout_path = thread
                .rollout_path
                .ok_or_else(|| ThreadStoreError::Internal {
                    message: format!("thread {} does not have a rollout path", params.thread_id),
                })?;
            (
                rollout_path,
                history.or_else(|| thread.history.map(|history| history.items)),
            )
        }
    };
    let state_builder = history
        .as_deref()
        .and_then(|items| builder_from_items(items, rollout_path.as_path()));
    let state_db_ctx = state_db::init(&store.config).await;
    let recorder = RolloutRecorder::new(
        &store.config,
        RolloutRecorderParams::resume(
            rollout_path,
            create_thread::event_persistence_mode(params.event_persistence_mode),
        ),
        state_db_ctx,
        state_builder,
    )
    .await
    .map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to resume local thread recorder: {err}"),
    })?;
    store.insert_live_recorder(params.thread_id, recorder).await;
    Ok(())
}

pub(super) async fn append_items(
    store: &LocalThreadStore,
    params: AppendThreadItemsParams,
) -> ThreadStoreResult<()> {
    store
        .live_recorder(params.thread_id)
        .await?
        .record_items(params.items.as_slice())
        .await
        .map_err(thread_store_io_error)
}

pub(super) async fn persist_thread(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<()> {
    store
        .live_recorder(thread_id)
        .await?
        .persist()
        .await
        .map_err(thread_store_io_error)
}

pub(super) async fn flush_thread(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<()> {
    store
        .live_recorder(thread_id)
        .await?
        .flush()
        .await
        .map_err(thread_store_io_error)
}

pub(super) async fn shutdown_thread(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<()> {
    let recorder = store.live_recorder(thread_id).await?;
    recorder.shutdown().await.map_err(thread_store_io_error)?;
    store.live_recorders.lock().await.remove(&thread_id);
    Ok(())
}

pub(super) async fn discard_thread(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<()> {
    store
        .live_recorders
        .lock()
        .await
        .remove(&thread_id)
        .map(|_| ())
        .ok_or(ThreadStoreError::ThreadNotFound { thread_id })
}

pub(super) async fn rollout_path(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<PathBuf> {
    Ok(store
        .live_recorders
        .lock()
        .await
        .get(&thread_id)
        .ok_or(ThreadStoreError::ThreadNotFound { thread_id })?
        .rollout_path()
        .to_path_buf())
}

fn thread_store_io_error(err: std::io::Error) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: err.to_string(),
    }
}
