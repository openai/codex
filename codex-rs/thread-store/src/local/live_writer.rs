use std::path::Path;
use std::path::PathBuf;

use codex_protocol::SegmentId;
use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutReferenceItem;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_rollout::RolloutConfig;
use codex_rollout::RolloutRecorder;
use codex_rollout::RolloutRecorderParams;
use codex_rollout::read_session_meta_line;
use tokio::fs;
use tracing::warn;

use super::LocalThreadStore;
use super::create_thread;
use crate::AppendThreadItemsParams;
use crate::CreateThreadParams;
use crate::ReadThreadParams;
use crate::ResumeThreadParams;
use crate::RotateThreadSegmentParams;
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
    let recorder = store.live_recorder(params.thread_id).await?;
    recorder
        .record_canonical_items(params.items.as_slice())
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
    store
        .live_recorder(thread_id)
        .await?
        .persist()
        .await
        .map_err(thread_store_io_error)?;
    sync_materialized_rollout_path(store, thread_id).await
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
        .map_err(thread_store_io_error)?;
    sync_materialized_rollout_path(store, thread_id).await
}

pub(super) async fn shutdown_thread(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<()> {
    let recorder = store.live_recorder(thread_id).await?;
    recorder.shutdown().await.map_err(thread_store_io_error)?;
    sync_materialized_rollout_path(store, thread_id).await?;
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

pub(super) async fn rotate_thread_segment(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    params: RotateThreadSegmentParams,
) -> ThreadStoreResult<()> {
    let old_recorder = store.live_recorder(thread_id).await?;
    old_recorder.flush().await.map_err(thread_store_io_error)?;
    let old_rollout_path = old_recorder.rollout_path().to_path_buf();
    let old_meta = read_session_meta_line(old_rollout_path.as_path())
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!(
                "failed to read current rollout metadata from {}: {err}",
                old_rollout_path.display()
            ),
        })?;
    if old_meta.meta.id != thread_id {
        return Err(ThreadStoreError::Internal {
            message: format!(
                "live rollout {} belongs to thread {} instead of {thread_id}",
                old_rollout_path.display(),
                old_meta.meta.id
            ),
        });
    }

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
    if let Err(err) = old_recorder.shutdown().await {
        warn!(
            "failed to close previous rollout segment {} for thread {thread_id}: {err}",
            old_rollout_path.display()
        );
    }

    let current_path = store
        .live_recorders
        .lock()
        .await
        .get(&thread_id)
        .ok_or(ThreadStoreError::ThreadNotFound { thread_id })?
        .rollout_path()
        .to_path_buf();
    if current_path != old_rollout_path {
        return Err(ThreadStoreError::Conflict {
            message: format!("live writer for thread {thread_id} changed during segment rotation"),
        });
    }

    let rotated_segment_path = rotated_segment_path(
        store.config.codex_home.as_path(),
        thread_id,
        old_meta.meta.segment_id,
        old_rollout_path.as_path(),
    )?;
    fs::create_dir_all(rotated_segment_path.parent().ok_or_else(|| {
        ThreadStoreError::Internal {
            message: format!(
                "rotated rollout segment path {} does not have a parent",
                rotated_segment_path.display()
            ),
        }
    })?)
    .await
    .map_err(thread_store_io_error)?;
    fs::copy(old_rollout_path.as_path(), rotated_segment_path.as_path())
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!(
                "failed to copy previous rollout segment {} to {}: {err}",
                old_rollout_path.display(),
                rotated_segment_path.display()
            ),
        })?;

    let mut initial_items = Vec::with_capacity(params.initial_items.len() + 1);
    initial_items.push(RolloutItem::RolloutReference(RolloutReferenceItem {
        rollout_path: rotated_segment_path.clone(),
        thread_id: Some(thread_id),
        rollout_timestamp: rollout_timestamp_from_path(old_rollout_path.as_path()),
        segment_id: old_meta.meta.segment_id,
        max_depth: params.previous_segment_reference_depth,
        nth_user_message: None,
        filter_fork_history: false,
        developer_message_filter_texts: None,
    }));
    initial_items.extend(params.initial_items);

    let staged_rollout_path = staged_rollout_path(old_rollout_path.as_path())?;
    let staged_recorder = match RolloutRecorder::new(
        &config,
        RolloutRecorderParams::CreateAtPath {
            path: staged_rollout_path.clone(),
            conversation_id: thread_id,
            forked_from_id: old_meta.meta.forked_from_id,
            parent_thread_id: old_meta.meta.parent_thread_id,
            source: params.source,
            thread_source: old_meta.meta.thread_source,
            base_instructions: params.base_instructions,
            dynamic_tools: params.dynamic_tools,
            multi_agent_version: old_meta.meta.multi_agent_version,
            session_timestamp: Some(old_meta.meta.timestamp.clone()),
        },
    )
    .await
    {
        Ok(staged_recorder) => staged_recorder,
        Err(err) => {
            remove_rotation_artifacts(
                staged_rollout_path.as_path(),
                rotated_segment_path.as_path(),
                "staged recorder initialization",
            )
            .await;
            return Err(ThreadStoreError::Internal {
                message: format!("failed to initialize rotated local thread recorder: {err}"),
            });
        }
    };
    if let Err(err) = staged_recorder
        .record_canonical_items(initial_items.as_slice())
        .await
    {
        let _ = staged_recorder.shutdown().await;
        remove_rotation_artifacts(
            staged_rollout_path.as_path(),
            rotated_segment_path.as_path(),
            "staged recorder write",
        )
        .await;
        return Err(thread_store_io_error(err));
    }
    if let Err(err) = staged_recorder.flush().await {
        let _ = staged_recorder.shutdown().await;
        remove_rotation_artifacts(
            staged_rollout_path.as_path(),
            rotated_segment_path.as_path(),
            "staged recorder flush",
        )
        .await;
        return Err(thread_store_io_error(err));
    }
    if let Err(err) = staged_recorder.shutdown().await {
        remove_rotation_artifacts(
            staged_rollout_path.as_path(),
            rotated_segment_path.as_path(),
            "staged recorder shutdown",
        )
        .await;
        return Err(thread_store_io_error(err));
    }

    if let Err(err) = replace_live_rollout_with_staged_segment(
        staged_rollout_path.as_path(),
        old_rollout_path.as_path(),
    )
    .await
    {
        remove_rotation_artifacts(
            staged_rollout_path.as_path(),
            rotated_segment_path.as_path(),
            "staged recorder install",
        )
        .await;
        return Err(err);
    }

    let new_recorder = RolloutRecorder::new(
        &config,
        RolloutRecorderParams::resume(old_rollout_path.clone()),
    )
    .await
    .map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to resume rotated local thread recorder: {err}"),
    })?;

    let mut live_recorders = store.live_recorders.lock().await;
    let current_path = live_recorders
        .get(&thread_id)
        .ok_or(ThreadStoreError::ThreadNotFound { thread_id })?
        .rollout_path()
        .to_path_buf();
    if current_path != old_rollout_path {
        return Err(ThreadStoreError::Conflict {
            message: format!("live writer for thread {thread_id} changed during segment rotation"),
        });
    }
    live_recorders.insert(thread_id, new_recorder);
    Ok(())
}

async fn sync_materialized_rollout_path(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<()> {
    let rollout_path = rollout_path(store, thread_id).await?;
    if codex_rollout::existing_rollout_path(rollout_path.as_path())
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
            metadata.rollout_path = rollout_path;
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

fn thread_store_io_error(err: std::io::Error) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: err.to_string(),
    }
}

fn rotated_segment_path(
    codex_home: &Path,
    thread_id: ThreadId,
    segment_id: Option<SegmentId>,
    old_rollout_path: &Path,
) -> ThreadStoreResult<PathBuf> {
    let old_file_name = old_rollout_path
        .file_name()
        .ok_or_else(|| ThreadStoreError::Internal {
            message: format!(
                "previous rollout segment path {} does not have a file name",
                old_rollout_path.display()
            ),
        })?;
    let segment_key = segment_id
        .map(|segment_id| segment_id.to_string())
        .unwrap_or_else(|| "initial".to_string());
    Ok(codex_home
        .join(codex_rollout::ROTATED_ROLLOUT_SEGMENTS_SUBDIR)
        .join(thread_id.to_string())
        .join(segment_key)
        .join(old_file_name))
}

fn staged_rollout_path(live_rollout_path: &Path) -> ThreadStoreResult<PathBuf> {
    let file_name = live_rollout_path
        .file_name()
        .ok_or_else(|| ThreadStoreError::Internal {
            message: format!(
                "live rollout path {} does not have a file name",
                live_rollout_path.display()
            ),
        })?;
    let mut staged_file_name = file_name.to_os_string();
    staged_file_name.push(format!(".staged-{}.tmp", SegmentId::new()));
    Ok(live_rollout_path.with_file_name(staged_file_name))
}

async fn replace_live_rollout_with_staged_segment(
    staged_rollout_path: &Path,
    live_rollout_path: &Path,
) -> ThreadStoreResult<()> {
    match fs::rename(staged_rollout_path, live_rollout_path).await {
        Ok(()) => Ok(()),
        Err(rename_err) => {
            fs::copy(staged_rollout_path, live_rollout_path)
                .await
                .map_err(|copy_err| ThreadStoreError::Internal {
                    message: format!(
                        "failed to replace live rollout {} from staged rollout {}: rename failed: {rename_err}; copy failed: {copy_err}",
                        live_rollout_path.display(),
                        staged_rollout_path.display()
                    ),
                })?;
            fs::remove_file(staged_rollout_path)
                .await
                .map_err(|remove_err| ThreadStoreError::Internal {
                    message: format!(
                        "failed to remove staged rollout {} after copying it to {}: {remove_err}",
                        staged_rollout_path.display(),
                        live_rollout_path.display()
                    ),
                })?;
            Ok(())
        }
    }
}

async fn remove_rotation_artifacts(staged_rollout_path: &Path, archived_path: &Path, stage: &str) {
    for path in [staged_rollout_path, archived_path] {
        if fs::try_exists(path).await.unwrap_or(false)
            && let Err(err) = fs::remove_file(path).await
        {
            warn!(
                "failed to remove rollout rotation artifact {} after {stage}: {err}",
                path.display()
            );
        }
    }
}

fn rollout_timestamp_from_path(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    let core = file_name.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;
    core.match_indices('-').rev().find_map(|(index, _)| {
        ThreadId::from_string(&core[index + 1..])
            .ok()
            .map(|_| core[..index].to_string())
    })
}
