use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_protocol::ThreadId;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_rollout::RolloutConfig;
use codex_rollout::RolloutRecorder;
use codex_rollout::RolloutRecorderParams;
use codex_rollout::persisted_rollout_items;
use tracing::warn;

use super::LiveRecorderHandle;
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
    store.ensure_live_recorder_absent(thread_id).await?;
    let recorder = create_thread::create_thread(store, params).await?;
    store.insert_live_recorder(thread_id, recorder).await
}

pub(super) async fn resume_thread(
    store: &LocalThreadStore,
    params: ResumeThreadParams,
) -> ThreadStoreResult<()> {
    store.ensure_live_recorder_absent(params.thread_id).await?;
    let rollout_path = match (params.rollout_path, params.history) {
        (Some(rollout_path), _history) => rollout_path,
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

            thread
                .rollout_path
                .ok_or_else(|| ThreadStoreError::Internal {
                    message: format!("thread {} does not have a rollout path", params.thread_id),
                })?
        }
    };
    let cwd = params
        .metadata
        .cwd
        .clone()
        .ok_or_else(|| ThreadStoreError::InvalidRequest {
            message: "local thread store requires a cwd".to_string(),
        })?;
    let config = RolloutConfig {
        codex_home: store.config.codex_home.clone(),
        sqlite_home: store.config.sqlite_home.clone(),
        cwd,
        model_provider_id: params.metadata.model_provider.clone(),
        generate_memories: matches!(params.metadata.memory_mode, ThreadMemoryMode::Enabled),
    };
    let recorder = RolloutRecorder::new(&config, RolloutRecorderParams::resume(rollout_path))
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to resume local thread recorder: {err}"),
        })?;
    store.insert_live_recorder(params.thread_id, recorder).await
}

pub(super) async fn append_items(
    store: &LocalThreadStore,
    params: AppendThreadItemsParams,
) -> ThreadStoreResult<()> {
    let canonical_items = persisted_rollout_items(params.items.as_slice());
    if canonical_items.is_empty() {
        return Ok(());
    }
    let handle = store.live_recorder_handle(params.thread_id).await?;
    let state = Arc::clone(&handle).lock_owned().await;
    let recorder = live_recorder(&state.recorder, params.thread_id)?;
    recorder
        .record_canonical_items(canonical_items.as_slice())
        .await
        .map_err(thread_store_io_error)?;
    // LiveThread applies metadata immediately after append_items returns. Wait for the local
    // writer so SQLite never gets ahead of JSONL for accepted live appends.
    recorder.flush().await.map_err(thread_store_io_error)
}

pub(super) async fn persist_thread(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<()> {
    let handle = store.live_recorder_handle(thread_id).await?;
    let rollout_path = {
        let state = Arc::clone(&handle).lock_owned().await;
        let recorder = live_recorder(&state.recorder, thread_id)?;
        recorder.persist().await.map_err(thread_store_io_error)?;
        recorder.rollout_path().to_path_buf()
    };
    sync_materialized_rollout_path(store, thread_id, rollout_path.as_path()).await
}

pub(super) async fn flush_thread(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<()> {
    let handle = store.live_recorder_handle(thread_id).await?;
    let rollout_path = {
        let state = Arc::clone(&handle).lock_owned().await;
        let recorder = live_recorder(&state.recorder, thread_id)?;
        recorder.flush().await.map_err(thread_store_io_error)?;
        recorder.rollout_path().to_path_buf()
    };
    sync_materialized_rollout_path(store, thread_id, rollout_path.as_path()).await
}

pub(super) async fn shutdown_thread(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<()> {
    let handle = store.live_recorder_handle(thread_id).await?;
    let rollout_path = {
        let mut state = Arc::clone(&handle).lock_owned().await;
        let recorder = state
            .recorder
            .take()
            .ok_or(ThreadStoreError::ThreadNotFound { thread_id })?;
        let rollout_path = recorder.rollout_path().to_path_buf();
        recorder.shutdown().await.map_err(thread_store_io_error)?;
        rollout_path
    };
    sync_materialized_rollout_path(store, thread_id, rollout_path.as_path()).await?;
    remove_live_recorder_handle(store, thread_id, &handle).await;
    Ok(())
}

pub(super) async fn discard_thread(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<()> {
    let handle = store.live_recorder_handle(thread_id).await?;
    {
        let mut state = handle.lock().await;
        state.recorder.take();
    }
    remove_live_recorder_handle(store, thread_id, &handle).await;
    Ok(())
}

pub(super) async fn close_thread_for_delete(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<()> {
    let Some(handle) = store.live_recorders.lock().await.get(&thread_id).cloned() else {
        return Ok(());
    };
    let recorder = handle.lock().await.recorder.take();
    if let Some(recorder) = recorder {
        recorder.shutdown().await.map_err(thread_store_io_error)?;
    }
    remove_live_recorder_handle(store, thread_id, &handle).await;
    Ok(())
}

pub(super) async fn rollout_path(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<PathBuf> {
    let handle = store.live_recorder_handle(thread_id).await?;
    let state = handle.lock().await;
    Ok(live_recorder(&state.recorder, thread_id)?
        .rollout_path()
        .to_path_buf())
}

pub(super) fn live_recorder(
    recorder: &Option<RolloutRecorder>,
    thread_id: ThreadId,
) -> ThreadStoreResult<&RolloutRecorder> {
    recorder
        .as_ref()
        .ok_or(ThreadStoreError::ThreadNotFound { thread_id })
}

pub(super) fn thread_store_io_error(err: std::io::Error) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: err.to_string(),
    }
}

pub(super) async fn remove_live_recorder_handle(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    handle: &LiveRecorderHandle,
) {
    let mut live_recorders = store.live_recorders.lock().await;
    if live_recorders
        .get(&thread_id)
        .is_some_and(|current| Arc::ptr_eq(current, handle))
    {
        live_recorders.remove(&thread_id);
    }
}

pub(super) async fn sync_materialized_rollout_path(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    rollout_path: &Path,
) -> ThreadStoreResult<()> {
    if codex_rollout::existing_rollout_path(rollout_path)
        .await
        .is_none()
    {
        return Ok(());
    }
    let Some(state_db) = store.state_db().await else {
        return Ok(());
    };
    let result: ThreadStoreResult<()> = async {
        let Some(mut metadata) =
            state_db
                .get_thread(thread_id)
                .await
                .map_err(|err| ThreadStoreError::Internal {
                    message: format!("failed to read thread metadata for {thread_id}: {err}"),
                })?
        else {
            return Ok(());
        };
        if metadata.rollout_path != rollout_path {
            metadata.rollout_path = rollout_path.to_path_buf();
            state_db
                .upsert_thread(&metadata)
                .await
                .map_err(|err| ThreadStoreError::Internal {
                    message: format!("failed to update thread metadata for {thread_id}: {err}"),
                })?;
        }
        Ok(())
    }
    .await;
    if let Err(err) = result {
        warn!("failed to sync materialized rollout path for thread {thread_id}: {err}");
    }
    Ok(())
}
