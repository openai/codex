use super::LocalThreadStore;
use super::helpers::owned_rollout_paths;
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
use codex_rollout::rollout_date_parts;

use super::thread_rollout_resolver;
use super::thread_rollout_resolver::RolloutLocation;

pub(super) async fn unarchive_thread(
    store: &LocalThreadStore,
    params: ArchiveThreadParams,
) -> ThreadStoreResult<StoredThread> {
    let thread_id = params.thread_id;
    let _lifecycle_guard = store.live_writer_locks.lock_lifecycle(thread_id).await;
    // Archive, delete, and revert use the same cross-process lock while moving or selecting
    // rollout files. Unarchive must participate before it moves those files back.
    let _writer_lock = store.writer_lock_coordinator.acquire(thread_id)?;
    let state_db_ctx = store.state_db().await;
    let selected_archived_path =
        thread_rollout_resolver::resolve_current_including_archived(store, thread_id)
            .await?
            .filter(|resolved| resolved.location == RolloutLocation::Archived)
            .map(|resolved| resolved.path)
            .ok_or_else(|| ThreadStoreError::InvalidRequest {
                message: format!("no archived rollout found for thread id {thread_id}"),
            })?;

    let mut rollout_paths = owned_rollout_paths(store, thread_id).await?;
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
        rollout_moves.push((canonical_archived_path, destination));
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

    super::read_thread::read_thread(
        store,
        ReadThreadParams {
            thread_id,
            include_archived: false,
            include_history: false,
        },
    )
    .await
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
            .writer_lock_coordinator
            .acquire(thread_id)
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
    async fn unarchive_thread_updates_sqlite_metadata_when_present() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let uuid = Uuid::from_u128(204);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let archived_path = write_archived_session_file(home.path(), "2025-01-03T13-00-00", uuid)
            .expect("archived session file");
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
            archived_path.clone(),
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

        let unarchived = store
            .unarchive_thread(ArchiveThreadParams { thread_id })
            .await
            .expect("unarchive thread");
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
    }
}
