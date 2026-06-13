//! Atomic local rollout-segment rotation.
//!
//! The live recorder mutex serializes appends with rotation. Rotation publishes the predecessor
//! at a new immutable path, writes the complete successor to a temporary file in the canonical
//! file's directory, and atomically replaces the canonical file. The canonical path therefore
//! always names either the complete predecessor or the complete successor. Immutable segment
//! lifetime is managed by `segment_gc` from references in canonical rollouts.

use std::collections::hash_map::Entry;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_protocol::SegmentId;
use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutReferenceItem;
use codex_rollout::RolloutConfig;
use codex_rollout::RolloutRecorder;
use codex_rollout::RolloutRecorderParams;
use codex_rollout::read_session_meta_line;
use tempfile::TempPath;
use tokio::fs;
use tracing::warn;

use super::LiveRecorderHandle;
use super::LiveRecorderState;
use super::LocalThreadStore;
use super::live_writer::live_recorder;
use super::live_writer::remove_live_recorder_handle;
use super::live_writer::sync_materialized_rollout_path;
use super::live_writer::thread_store_io_error;
use crate::RotateThreadSegmentParams;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

#[derive(Clone, Copy)]
enum ReferenceBoundary {
    Segment,
    Snapshot,
}

pub(super) async fn rotate_thread_segment(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    params: RotateThreadSegmentParams,
) -> ThreadStoreResult<RolloutReferenceItem> {
    let handle = store.live_recorder_handle(thread_id).await?;
    Box::pin(rotate_thread_segment_with_handle(
        store,
        thread_id,
        params,
        handle,
        ReferenceBoundary::Segment,
    ))
    .await
}

pub(super) async fn seal_live_thread_segment(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    params: RotateThreadSegmentParams,
) -> ThreadStoreResult<RolloutReferenceItem> {
    let handle = store.live_recorder_handle(thread_id).await?;
    Box::pin(rotate_thread_segment_with_handle(
        store,
        thread_id,
        params,
        handle,
        ReferenceBoundary::Snapshot,
    ))
    .await
}

pub(super) async fn seal_thread_segment(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    rollout_path: PathBuf,
    params: RotateThreadSegmentParams,
) -> ThreadStoreResult<RolloutReferenceItem> {
    let existing_handle = store.live_recorders.lock().await.get(&thread_id).cloned();
    if let Some(handle) = existing_handle {
        ensure_requested_rollout(&handle, thread_id, rollout_path.as_path()).await?;
        let reference = Box::pin(rotate_thread_segment_with_handle(
            store,
            thread_id,
            params,
            handle,
            ReferenceBoundary::Snapshot,
        ))
        .await?;
        let canonical_path = codex_rollout::plain_rollout_path(rollout_path.as_path());
        sync_materialized_rollout_path(store, thread_id, canonical_path.as_path()).await?;
        return Ok(reference);
    }

    let source_meta = read_session_meta_line(rollout_path.as_path())
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!(
                "failed to read rollout metadata from {} before snapshot: {err}",
                rollout_path.display()
            ),
        })?;
    let config = rollout_config(store, &source_meta.meta);
    let candidate =
        RolloutRecorder::new(&config, RolloutRecorderParams::resume(rollout_path.clone()))
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!(
                    "failed to open rollout {} for snapshot: {err}",
                    rollout_path.display()
                ),
            })?;
    let candidate_handle = Arc::new(tokio::sync::Mutex::new(LiveRecorderState {
        recorder: Some(candidate.clone()),
    }));
    let (handle, owns_handle) = {
        let mut live_recorders = store.live_recorders.lock().await;
        match live_recorders.entry(thread_id) {
            Entry::Occupied(entry) => (Arc::clone(entry.get()), false),
            Entry::Vacant(entry) => {
                entry.insert(Arc::clone(&candidate_handle));
                (candidate_handle, true)
            }
        }
    };
    if !owns_handle {
        let _ = candidate.shutdown().await;
        ensure_requested_rollout(&handle, thread_id, rollout_path.as_path()).await?;
    }

    let result = Box::pin(rotate_thread_segment_with_handle(
        store,
        thread_id,
        params,
        Arc::clone(&handle),
        ReferenceBoundary::Snapshot,
    ))
    .await;
    if owns_handle {
        let recorder = handle.lock().await.recorder.take();
        if let Some(recorder) = recorder
            && let Err(err) = recorder.shutdown().await
        {
            warn!("failed to close snapshot-only writer for thread {thread_id}: {err}");
        }
        remove_live_recorder_handle(store, thread_id, &handle).await;
    }
    if result.is_ok() {
        let canonical_path = codex_rollout::plain_rollout_path(rollout_path.as_path());
        sync_materialized_rollout_path(store, thread_id, canonical_path.as_path()).await?;
    }
    result
}

async fn ensure_requested_rollout(
    handle: &LiveRecorderHandle,
    thread_id: ThreadId,
    requested_path: &Path,
) -> ThreadStoreResult<()> {
    let current_path = {
        let state = handle.lock().await;
        live_recorder(&state.recorder, thread_id)?
            .rollout_path()
            .to_path_buf()
    };
    if codex_rollout::plain_rollout_path(current_path.as_path())
        == codex_rollout::plain_rollout_path(requested_path)
    {
        return Ok(());
    }
    Err(ThreadStoreError::Conflict {
        message: format!(
            "thread {thread_id} is running from {} instead of requested rollout {}",
            current_path.display(),
            requested_path.display()
        ),
    })
}

async fn rotate_thread_segment_with_handle(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    params: RotateThreadSegmentParams,
    handle: LiveRecorderHandle,
    reference_boundary: ReferenceBoundary,
) -> ThreadStoreResult<RolloutReferenceItem> {
    let mut state = handle.lock_owned().await;
    let _segment_storage_guard = Arc::clone(&store.segment_storage_lock).lock_owned().await;
    let old_recorder = live_recorder(&state.recorder, thread_id)?.clone();
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

    let config = rollout_config(store, &old_meta.meta);
    let immutable_path = rotated_segment_path(
        store.config.codex_home.as_path(),
        thread_id,
        old_meta.meta.segment_id,
        old_rollout_path.as_path(),
    )?;
    remove_unreferenced_immutable_collision(store, immutable_path.as_path()).await?;
    copy_rollout_to_immutable_path(old_rollout_path.as_path(), immutable_path.as_path()).await?;

    let reference = RolloutReferenceItem {
        rollout_path: immutable_path.clone(),
        thread_id: Some(thread_id),
        rollout_timestamp: rollout_timestamp_from_path(old_rollout_path.as_path()),
        segment_id: old_meta.meta.segment_id,
        max_depth: params.previous_segment_reference_depth,
        nth_user_message: match reference_boundary {
            ReferenceBoundary::Segment => None,
            ReferenceBoundary::Snapshot => Some(usize::MAX),
        },
        compacted_replacement_history_filter_texts: None,
    };
    let mut initial_items = Vec::with_capacity(params.initial_items.len() + 1);
    initial_items.push(RolloutItem::RolloutReference(reference.clone()));
    initial_items.extend(params.initial_items);

    let staged_rollout_path =
        create_staged_rollout_path(old_rollout_path.as_path()).map_err(thread_store_io_error)?;
    let staged_recorder = match RolloutRecorder::new(
        &config,
        RolloutRecorderParams::CreateAtPath {
            path: staged_rollout_path.to_path_buf(),
            conversation_id: thread_id,
            forked_from_id: old_meta.meta.forked_from_id,
            parent_thread_id: old_meta.meta.parent_thread_id,
            source: old_meta.meta.source.clone(),
            thread_source: old_meta.meta.thread_source,
            base_instructions: old_meta.meta.base_instructions.clone().unwrap_or_default(),
            dynamic_tools: old_meta.meta.dynamic_tools.clone().unwrap_or_default(),
            multi_agent_version: old_meta.meta.multi_agent_version,
            session_timestamp: Some(old_meta.meta.timestamp.clone()),
        },
    )
    .await
    {
        Ok(staged_recorder) => staged_recorder,
        Err(err) => {
            remove_immutable_after_failed_rotation(immutable_path.as_path()).await;
            return Err(ThreadStoreError::Internal {
                message: format!("failed to initialize rotated local thread recorder: {err}"),
            });
        }
    };
    if let Err(err) = write_staged_rollout(&staged_recorder, initial_items.as_slice()).await {
        remove_immutable_after_failed_rotation(immutable_path.as_path()).await;
        return Err(err);
    }

    if let Err(err) = old_recorder.shutdown().await {
        restore_recorder_at_path(
            &mut state.recorder,
            &config,
            thread_id,
            old_rollout_path.as_path(),
        )
        .await;
        remove_immutable_after_failed_rotation(immutable_path.as_path()).await;
        return Err(ThreadStoreError::Internal {
            message: format!(
                "failed to close previous rollout segment {} for thread {thread_id}: {err}",
                old_rollout_path.display()
            ),
        });
    }

    #[cfg(test)]
    {
        let hook = store.rotation_test_hook.lock().await.clone();
        if let Some(hook) = hook {
            hook.reached.wait().await;
            hook.release.wait().await;
            if hook.fail_before_install {
                restore_recorder_at_path(
                    &mut state.recorder,
                    &config,
                    thread_id,
                    old_rollout_path.as_path(),
                )
                .await;
                remove_immutable_after_failed_rotation(immutable_path.as_path()).await;
                return Err(ThreadStoreError::Internal {
                    message: "injected rollout rotation failure before atomic install".to_string(),
                });
            }
        }
    }

    if let Err(err) = staged_rollout_path.persist(old_rollout_path.as_path()) {
        restore_recorder_at_path(
            &mut state.recorder,
            &config,
            thread_id,
            old_rollout_path.as_path(),
        )
        .await;
        remove_immutable_after_failed_rotation(immutable_path.as_path()).await;
        return Err(ThreadStoreError::Internal {
            message: format!(
                "failed to atomically replace live rollout {}: {}",
                old_rollout_path.display(),
                err.error
            ),
        });
    }

    match RolloutRecorder::new(
        &config,
        RolloutRecorderParams::resume(old_rollout_path.clone()),
    )
    .await
    {
        Ok(new_recorder) => {
            state.recorder = Some(new_recorder);
            Ok(reference)
        }
        Err(resume_err) => {
            let restore_result = restore_previous_rollout_after_install(
                &config,
                old_rollout_path.as_path(),
                immutable_path.as_path(),
            )
            .await;
            match restore_result {
                Ok(restored_recorder) => {
                    state.recorder = Some(restored_recorder);
                    remove_immutable_after_failed_rotation(immutable_path.as_path()).await;
                    Err(ThreadStoreError::Internal {
                        message: format!(
                            "failed to resume rotated local thread recorder; restored the previous segment: {resume_err}"
                        ),
                    })
                }
                Err(restore_err) => {
                    state.recorder = None;
                    Err(ThreadStoreError::Internal {
                        message: format!(
                            "failed to resume rotated local thread recorder: {resume_err}; failed to restore previous segment: {restore_err}"
                        ),
                    })
                }
            }
        }
    }
}

fn rollout_config(store: &LocalThreadStore, meta: &codex_rollout::SessionMeta) -> RolloutConfig {
    RolloutConfig {
        codex_home: store.config.codex_home.clone(),
        sqlite_home: store.config.sqlite_home.clone(),
        cwd: meta.cwd.clone(),
        model_provider_id: meta
            .model_provider
            .clone()
            .unwrap_or_else(|| store.config.default_model_provider_id.clone()),
        generate_memories: meta.memory_mode.as_deref() != Some("disabled"),
    }
}

async fn write_staged_rollout(
    recorder: &RolloutRecorder,
    initial_items: &[RolloutItem],
) -> ThreadStoreResult<()> {
    if let Err(err) = recorder.record_canonical_items(initial_items).await {
        let _ = recorder.shutdown().await;
        return Err(thread_store_io_error(err));
    }
    if let Err(err) = recorder.flush().await {
        let _ = recorder.shutdown().await;
        return Err(thread_store_io_error(err));
    }
    recorder.shutdown().await.map_err(thread_store_io_error)
}

async fn restore_recorder_at_path(
    recorder: &mut Option<RolloutRecorder>,
    config: &RolloutConfig,
    thread_id: ThreadId,
    rollout_path: &Path,
) {
    match RolloutRecorder::new(
        config,
        RolloutRecorderParams::resume(rollout_path.to_path_buf()),
    )
    .await
    {
        Ok(restored) => *recorder = Some(restored),
        Err(err) => {
            *recorder = None;
            warn!(
                "failed to restore live rollout recorder {} for thread {thread_id}: {err}",
                rollout_path.display()
            );
        }
    }
}

async fn restore_previous_rollout_after_install(
    config: &RolloutConfig,
    live_rollout_path: &Path,
    immutable_path: &Path,
) -> io::Result<RolloutRecorder> {
    let rollback_path = create_staged_rollout_path(live_rollout_path)?;
    copy_file_contents(immutable_path, rollback_path.as_ref()).await?;
    rollback_path.persist(live_rollout_path)?;
    RolloutRecorder::new(
        config,
        RolloutRecorderParams::resume(live_rollout_path.to_path_buf()),
    )
    .await
}

async fn copy_rollout_to_immutable_path(
    source: &Path,
    destination: &Path,
) -> ThreadStoreResult<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| ThreadStoreError::Internal {
            message: format!(
                "rotated rollout segment path {} does not have a parent",
                destination.display()
            ),
        })?;
    fs::create_dir_all(parent)
        .await
        .map_err(thread_store_io_error)?;
    let staged_path = tempfile::NamedTempFile::new_in(parent)
        .map_err(thread_store_io_error)?
        .into_temp_path();
    copy_file_contents(source, staged_path.as_ref())
        .await
        .map_err(thread_store_io_error)?;
    staged_path
        .persist_noclobber(destination)
        .map_err(|err| ThreadStoreError::Internal {
            message: format!(
                "failed to publish immutable rollout segment {}: {}",
                destination.display(),
                err.error
            ),
        })
}

async fn remove_unreferenced_immutable_collision(
    store: &LocalThreadStore,
    immutable_path: &Path,
) -> ThreadStoreResult<()> {
    if !fs::try_exists(immutable_path)
        .await
        .map_err(thread_store_io_error)?
    {
        return Ok(());
    }
    super::segment_gc::collect_unreferenced_segments(store.config.codex_home.as_path())
        .await
        .map_err(thread_store_io_error)?;
    if fs::try_exists(immutable_path)
        .await
        .map_err(thread_store_io_error)?
    {
        return Err(ThreadStoreError::Conflict {
            message: format!(
                "immutable rollout segment {} is already referenced",
                immutable_path.display()
            ),
        });
    }
    Ok(())
}

async fn copy_file_contents(source: &Path, destination: &Path) -> io::Result<()> {
    fs::copy(source, destination).await?;
    fs::OpenOptions::new()
        .write(true)
        .open(destination)
        .await?
        .sync_all()
        .await
}

fn create_staged_rollout_path(live_rollout_path: &Path) -> io::Result<TempPath> {
    let parent = live_rollout_path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "live rollout path {} does not have a parent",
                live_rollout_path.display()
            ),
        )
    })?;
    Ok(tempfile::NamedTempFile::new_in(parent)?.into_temp_path())
}

async fn remove_immutable_after_failed_rotation(path: &Path) {
    if let Err(err) = fs::remove_file(path).await
        && err.kind() != io::ErrorKind::NotFound
    {
        warn!(
            "failed to remove immutable rollout after failed rotation {}: {err}",
            path.display()
        );
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

fn rollout_timestamp_from_path(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    let core = file_name.strip_prefix("rollout-")?.strip_suffix(".jsonl")?;
    core.match_indices('-').rev().find_map(|(index, _)| {
        ThreadId::from_string(&core[index + 1..])
            .ok()
            .map(|_| core[..index].to_string())
    })
}

#[cfg(test)]
#[path = "segment_writer_tests.rs"]
mod tests;
