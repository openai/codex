use super::LocalThreadStore;
use super::helpers::owned_rollout_paths_from_index;
use super::helpers::restore_rollout_moves;
use super::helpers::rollout_path_is_archived;
use super::helpers::scoped_rollout_path;
use super::helpers::touch_modified_time;
use super::helpers::validated_rollout_file_name;
use crate::ArchiveThreadParams;
use crate::ReadThreadParams;
use crate::StoredThread;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use codex_protocol::ThreadId;
use codex_rollout::RolloutReferenceIndex;
use codex_rollout::rollout_date_parts;
use tracing::warn;

use super::thread_rollout_resolver;
use super::thread_rollout_resolver::RolloutLocation;

pub(super) async fn unarchive_threads(
    store: &LocalThreadStore,
    params: ArchiveThreadParams,
    descendant_thread_ids: Vec<ThreadId>,
) -> ThreadStoreResult<(StoredThread, Vec<ThreadId>)> {
    let parent_thread_id = params.thread_id;
    let requested_ids: Vec<_> = std::iter::once(parent_thread_id)
        .chain(descendant_thread_ids)
        .collect();
    // Keep the index stable against other mutations in this store, using archive's lock order.
    let mut lock_thread_ids = requested_ids.clone();
    lock_thread_ids.sort_unstable_by_key(ToString::to_string);
    lock_thread_ids.dedup();
    let mut _lifecycle_guards = Vec::with_capacity(lock_thread_ids.len());
    for thread_id in lock_thread_ids {
        _lifecycle_guards.push(store.live_writer_locks.lock_lifecycle(thread_id).await);
    }
    // Hold ownership while discovering and moving rollouts, as archive does. A busy
    // descendant remains a best-effort failure rather than blocking the parent's restore.
    let mut thread_ids = Vec::new();
    let mut _writer_guards = Vec::new();
    for (index, thread_id) in requested_ids.into_iter().enumerate() {
        match store.acquire_writer_lock(thread_id) {
            Ok(guard) => {
                _writer_guards.push(guard);
                thread_ids.push(thread_id);
            }
            Err(err) if index == 0 => return Err(err),
            Err(err) => warn!("failed to unarchive spawned descendant thread {thread_id}: {err}"),
        }
    }
    // Like archive, discover candidate rollouts once for the entire subtree.
    let reference_index = RolloutReferenceIndex::scan_archived_threads(
        store.config.codex_home.as_path(),
        &thread_ids,
    )
    .await
    .map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to scan thread rollout files: {err}"),
    })?;
    unarchive_thread_with_index(store, parent_thread_id, &reference_index)
        .await?
        .ok_or_else(|| ThreadStoreError::InvalidRequest {
            message: format!("no archived rollout found for thread id {parent_thread_id}"),
        })?;
    let root = super::read_thread::read_thread(
        store,
        ReadThreadParams {
            thread_id: parent_thread_id,
            include_archived: false,
            include_history: false,
        },
    )
    .await?;
    let mut restored = vec![parent_thread_id];
    for thread_id in thread_ids.into_iter().skip(1) {
        match unarchive_thread_with_index(store, thread_id, &reference_index).await {
            Ok(Some(thread_id)) => restored.push(thread_id),
            Ok(None) => {}
            Err(err) => warn!("failed to unarchive spawned descendant thread {thread_id}: {err}"),
        }
    }
    Ok((root, restored))
}

async fn unarchive_thread_with_index(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    reference_index: &RolloutReferenceIndex,
) -> ThreadStoreResult<Option<ThreadId>> {
    let state_db_ctx = store.state_db().await;
    let Some(selected_archived_path) =
        thread_rollout_resolver::resolve_current_including_archived(store, thread_id)
            .await?
            .filter(|resolved| resolved.location == RolloutLocation::Archived)
            .map(|resolved| resolved.path)
    else {
        return Ok(None);
    };

    let mut rollout_paths = owned_rollout_paths_from_index(reference_index, thread_id);
    if !rollout_paths.contains(&selected_archived_path) {
        rollout_paths.push(selected_archived_path.clone());
    }
    let mut restored_path = None;
    let mut rollout_moves = Vec::new();
    for rollout_path in rollout_paths {
        if !rollout_path_is_archived(store.config.codex_home.as_path(), rollout_path.as_path()) {
            continue;
        }
        let canonical_archived_path = scoped_rollout_path(
            store
                .config
                .codex_home
                .join(codex_rollout::ARCHIVED_SESSIONS_SUBDIR),
            rollout_path.as_path(),
            "archived",
        )?;
        let file_name =
            validated_rollout_file_name(canonical_archived_path.as_path(), rollout_path.as_path())?;
        let Some((year, month, day)) = rollout_date_parts(&file_name) else {
            return Err(ThreadStoreError::InvalidRequest {
                message: format!(
                    "rollout path `{}` missing filename timestamp",
                    rollout_path.display()
                ),
            });
        };
        let dest_dir = store
            .config
            .codex_home
            .join(codex_rollout::SESSIONS_SUBDIR)
            .join(year)
            .join(month)
            .join(day);
        std::fs::create_dir_all(&dest_dir).map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to unarchive thread: {err}"),
        })?;
        let destination = dest_dir.join(&file_name);
        if rollout_path == selected_archived_path {
            restored_path = Some(destination.clone());
        }
        if !rollout_moves
            .iter()
            .any(|(source, _)| source == &canonical_archived_path)
        {
            rollout_moves.push((canonical_archived_path, destination));
        }
    }
    let restored_path = restored_path.ok_or_else(|| ThreadStoreError::Internal {
        message: format!("failed to unarchive selected rollout for thread {thread_id}"),
    })?;

    for (index, (source, destination)) in rollout_moves.iter().enumerate() {
        if let Err(err) = std::fs::rename(source, destination) {
            if let Err(restore_err) = restore_rollout_moves(&rollout_moves[..index]) {
                return Err(ThreadStoreError::Internal {
                    message: format!(
                        "failed to unarchive thread: {err}; failed to restore moved rollouts: {restore_err}"
                    ),
                });
            }
            return Err(ThreadStoreError::Internal {
                message: format!("failed to unarchive thread: {err}"),
            });
        }
    }
    if let Err(err) = touch_modified_time(restored_path.as_path()) {
        if let Err(restore_err) = restore_rollout_moves(&rollout_moves) {
            return Err(ThreadStoreError::Internal {
                message: format!(
                    "failed to update unarchived thread timestamp: {err}; failed to restore moved rollouts: {restore_err}"
                ),
            });
        }
        return Err(ThreadStoreError::Internal {
            message: format!("failed to update unarchived thread timestamp: {err}"),
        });
    }

    if let Some(ctx) = state_db_ctx
        && let Err(err) = ctx
            .mark_unarchived(thread_id, restored_path.as_path())
            .await
    {
        if let Err(restore_err) = restore_rollout_moves(&rollout_moves) {
            return Err(ThreadStoreError::Internal {
                message: format!(
                    "failed to update unarchived thread metadata: {err}; failed to restore moved rollouts: {restore_err}"
                ),
            });
        }
        return Err(ThreadStoreError::Internal {
            message: format!("failed to update unarchived thread metadata: {err}"),
        });
    }

    // Descendants need only their restored id for notifications, not metadata hydration.
    Ok(Some(thread_id))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::SessionSource;
    use codex_utils_absolute_path::test_support::PathExt;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;
    use crate::ThreadStore;
    use crate::local::LocalThreadStore;
    use crate::local::test_support::test_config;
    use crate::local::test_support::write_archived_session_file;
    use crate::local::test_support::write_session_file;

    #[tokio::test]
    async fn unarchive_thread_restores_rollout_and_returns_updated_thread() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let uuid = Uuid::from_u128(203);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let archived_path = write_archived_session_file(home.path(), "2025-01-03T13-00-00", uuid)
            .expect("archived session file");

        let thread = store
            .unarchive_thread(ArchiveThreadParams { thread_id })
            .await
            .expect("unarchive thread");

        assert!(!archived_path.exists());
        let restored_path = home
            .path()
            .join("sessions/2025/01/03")
            .join(archived_path.file_name().expect("file name"));
        assert!(restored_path.exists());
        assert_eq!(thread.thread_id, thread_id);
        assert_eq!(thread.rollout_path, Some(restored_path));
        assert_eq!(thread.archived_at, None);
        assert_eq!(thread.preview, "Archived user message");
        assert_eq!(
            thread.first_user_message.as_deref(),
            Some("Archived user message")
        );
    }

    #[tokio::test]
    async fn unarchive_thread_rejects_a_cross_process_writer() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let owner = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let uuid = Uuid::from_u128(205);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let archived_path = write_archived_session_file(home.path(), "2025-01-03T13-00-00", uuid)
            .expect("archived session file");
        let writer_guard = owner
            .acquire_writer_lock(thread_id)
            .expect("acquire writer lock");

        let error = store
            .unarchive_thread(ArchiveThreadParams { thread_id })
            .await
            .expect_err("active writer should block unarchive");

        assert!(matches!(error, ThreadStoreError::Conflict { .. }));
        assert!(archived_path.exists());
        drop(writer_guard);
        store
            .unarchive_thread(ArchiveThreadParams { thread_id })
            .await
            .expect("unarchive after writer exits");
    }

    #[tokio::test]
    async fn unarchive_thread_deduplicates_rollout_paths_and_updates_sqlite_metadata() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let uuid = Uuid::from_u128(204);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let archived_path = write_archived_session_file(home.path(), "2025-01-03T13-00-00", uuid)
            .expect("archived session file");
        let alternate_directory = archived_path
            .parent()
            .expect("archived session directory")
            .join("alternate");
        std::fs::create_dir(&alternate_directory).expect("alternate archived session directory");
        let selected_archived_path = alternate_directory
            .join("..")
            .join(archived_path.file_name().expect("file name"));
        let runtime = codex_state::StateRuntime::init(
            codex_state::SqliteConfig::new_for_testing(home.path().abs()),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config.clone(), Some(runtime.clone()));
        runtime
            .mark_backfill_complete(/*last_watermark*/ None)
            .await
            .expect("backfill should be complete");
        let mut builder = codex_state::ThreadMetadataBuilder::new(
            thread_id,
            selected_archived_path,
            Utc::now(),
            SessionSource::Cli,
        );
        builder.model_provider = Some(config.default_model_provider_id.clone());
        builder.cwd = home.path().to_path_buf();
        builder.cli_version = Some("test_version".to_string());
        let mut metadata = builder.build(config.default_model_provider_id.as_str());
        metadata.archived_at = Some(metadata.updated_at);
        metadata.section = Some(codex_state::ThreadSection {
            id: codex_state::PINNED_THREAD_SECTION_ID.to_string(),
            name: codex_state::PINNED_THREAD_SECTION_NAME.to_string(),
            appearance: None,
        });
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("state db upsert should succeed");

        let child_uuid = Uuid::from_u128(208);
        let child_id = ThreadId::from_string(&child_uuid.to_string()).expect("valid child id");
        let child_path =
            write_archived_session_file(home.path(), "2025-01-03T13-00-00", child_uuid)
                .expect("child rollout");
        let mut child_metadata = metadata.clone();
        child_metadata.id = child_id;
        child_metadata.rollout_path = child_path.clone();
        runtime
            .upsert_thread(&child_metadata)
            .await
            .expect("child metadata");
        std::fs::write(&child_path, "invalid session metadata\n").expect("malformed child rollout");

        let active_uuid = Uuid::from_u128(209);
        let active_id = ThreadId::from_string(&active_uuid.to_string()).expect("valid active id");
        let active_path = write_session_file(home.path(), "2025-01-03T13-00-00", active_uuid)
            .expect("active rollout");
        let (unarchived, restored_ids) = store
            .unarchive_threads(ArchiveThreadParams { thread_id }, vec![active_id, child_id])
            .await
            .expect("unarchive threads");
        assert!(active_path.is_file());
        assert_eq!(restored_ids, vec![thread_id, child_id]);
        assert!(!child_path.exists());
        assert_eq!(unarchived.section, metadata.section);

        let restored_path = home
            .path()
            .join("sessions/2025/01/03")
            .join(archived_path.file_name().expect("file name"));
        let updated = runtime
            .get_thread(thread_id)
            .await
            .expect("state db read should succeed")
            .expect("thread metadata should exist");
        assert_eq!(updated.rollout_path, restored_path);
        assert_eq!(updated.archived_at, None);
        assert_eq!(updated.recency_at, metadata.recency_at);
        assert_eq!(updated.section, metadata.section);

        let child = runtime.get_thread(child_id).await.unwrap().unwrap();
        assert_eq!(child.archived_at, None);
        assert_eq!(
            child.rollout_path,
            restored_path.with_file_name(child_path.file_name().unwrap())
        );
        assert!(child.rollout_path.is_file());
    }
}
