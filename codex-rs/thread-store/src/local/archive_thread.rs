use super::LocalThreadStore;
use super::helpers::matching_rollout_file_name;
use super::helpers::scoped_rollout_path;
use crate::ArchiveThreadParams;
use crate::ArchiveThreadsParams;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use chrono::Utc;
use codex_rollout::find_thread_path_by_id_str;
use tracing::warn;

pub(super) async fn archive_threads(
    store: &LocalThreadStore,
    params: ArchiveThreadsParams,
) -> ThreadStoreResult<Vec<codex_protocol::ThreadId>> {
    let thread_ids = params.thread_ids;
    if thread_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut lock_thread_ids = params.writer_lock_thread_ids;
    lock_thread_ids.extend(thread_ids.iter().copied());
    lock_thread_ids.sort_unstable_by_key(ToString::to_string);
    lock_thread_ids.dedup();
    let mut _live_writer_guards = Vec::with_capacity(lock_thread_ids.len());
    for thread_id in &lock_thread_ids {
        _live_writer_guards.push(store.live_writer_locks.lock(*thread_id).await);
        if store
            .live_recorders
            .lock()
            .await
            .get(thread_id)
            .is_some_and(|entry| entry.writer_lock.is_some())
        {
            return Err(ThreadStoreError::Conflict {
                message: format!("thread {thread_id} already has an active writer"),
            });
        }
    }
    let _writer_guards = store
        .acquire_paginated_writer_locks(&lock_thread_ids)
        .await?;

    let parent_thread_id = thread_ids[0];
    let mut archived_thread_ids = Vec::new();
    for thread_id in thread_ids {
        match archive_thread(store, ArchiveThreadParams { thread_id }).await {
            Ok(()) => archived_thread_ids.push(thread_id),
            Err(err) if archived_thread_ids.is_empty() => return Err(err),
            Err(err) => warn!(
                "failed to archive spawned descendant thread {thread_id} while archiving {parent_thread_id}: {err}"
            ),
        }
    }
    Ok(archived_thread_ids)
}

pub(super) async fn archive_thread(
    store: &LocalThreadStore,
    params: ArchiveThreadParams,
) -> ThreadStoreResult<()> {
    let thread_id = params.thread_id;
    let state_db_ctx = store.state_db().await;
    let rollout_path = find_thread_path_by_id_str(
        store.config.codex_home.as_path(),
        &thread_id.to_string(),
        state_db_ctx.as_deref(),
    )
    .await
    .map_err(|err| ThreadStoreError::InvalidRequest {
        message: format!("failed to locate thread id {thread_id}: {err}"),
    })?
    .ok_or_else(|| ThreadStoreError::InvalidRequest {
        message: format!("no rollout found for thread id {thread_id}"),
    })?;

    let canonical_rollout_path = scoped_rollout_path(
        store.config.codex_home.join(codex_rollout::SESSIONS_SUBDIR),
        rollout_path.as_path(),
        "sessions",
    )?;
    let file_name = matching_rollout_file_name(
        canonical_rollout_path.as_path(),
        thread_id,
        rollout_path.as_path(),
    )?;

    let archive_folder = store
        .config
        .codex_home
        .join(codex_rollout::ARCHIVED_SESSIONS_SUBDIR);
    std::fs::create_dir_all(&archive_folder).map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to archive thread: {err}"),
    })?;
    let archived_path = archive_folder.join(&file_name);
    std::fs::rename(&canonical_rollout_path, &archived_path).map_err(|err| {
        ThreadStoreError::Internal {
            message: format!("failed to archive thread: {err}"),
        }
    })?;

    if let Some(ctx) = state_db_ctx {
        let _ = ctx
            .mark_archived(thread_id, archived_path.as_path(), Utc::now())
            .await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::ThreadHistoryMode;
    use codex_rollout::ARCHIVED_SESSIONS_SUBDIR;
    use codex_utils_absolute_path::test_support::PathExt;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;
    use crate::ListThreadsParams;
    use crate::ThreadSortKey;
    use crate::ThreadStore;
    use crate::local::LocalThreadStore;
    use crate::local::test_support::test_config;
    use crate::local::test_support::write_session_file;
    use crate::local::test_support::write_session_file_with_history_mode;

    #[tokio::test]
    async fn archive_threads_rejects_owned_descendants_before_archiving_anything() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let owner = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let parent_uuid = Uuid::from_u128(203);
        let parent_thread_id =
            ThreadId::from_string(&parent_uuid.to_string()).expect("valid parent thread id");
        let parent_path = write_session_file_with_history_mode(
            home.path(),
            "2025-01-03T12-00-00",
            parent_uuid,
            ThreadHistoryMode::Paginated,
        )
        .expect("parent session file");
        let child_uuid = Uuid::from_u128(204);
        let child_thread_id =
            ThreadId::from_string(&child_uuid.to_string()).expect("valid child thread id");
        let child_path = write_session_file_with_history_mode(
            home.path(),
            "2025-01-03T12-00-01",
            child_uuid,
            ThreadHistoryMode::Paginated,
        )
        .expect("child session file");
        let _owner_guard = owner
            .writer_lock_coordinator
            .acquire(child_thread_id)
            .expect("acquire child writer lock");

        let error = store
            .archive_threads(ArchiveThreadsParams {
                thread_ids: vec![parent_thread_id, child_thread_id],
                writer_lock_thread_ids: Vec::new(),
            })
            .await
            .expect_err("owned descendant should block archive");

        assert!(matches!(error, ThreadStoreError::Conflict { .. }));
        assert!(parent_path.exists());
        assert!(child_path.exists());
    }

    #[tokio::test]
    async fn archive_thread_moves_rollout_to_archived_collection() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let uuid = Uuid::from_u128(201);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let active_path =
            write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");

        store
            .archive_thread(ArchiveThreadParams { thread_id })
            .await
            .expect("archive thread");

        assert!(!active_path.exists());
        let archived_path = home
            .path()
            .join(ARCHIVED_SESSIONS_SUBDIR)
            .join(active_path.file_name().expect("file name"));
        assert!(archived_path.exists());

        let archived = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::CreatedAt,
                sort_direction: crate::SortDirection::Desc,
                allowed_sources: Vec::new(),
                model_providers: None,
                cwd_filters: None,
                is_pinned: None,
                archived: true,
                search_term: None,
                relation_filter: None,
                use_state_db_only: false,
            })
            .await
            .expect("archived listing");
        assert_eq!(archived.items.len(), 1);
        assert_eq!(archived.items[0].thread_id, thread_id);
        assert_eq!(archived.items[0].rollout_path, Some(archived_path));
        assert_eq!(
            archived.items[0].archived_at,
            Some(archived.items[0].updated_at)
        );
    }

    #[tokio::test]
    async fn archive_thread_updates_sqlite_metadata_when_present() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let uuid = Uuid::from_u128(202);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let active_path =
            write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");
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
            active_path.clone(),
            Utc::now(),
            SessionSource::Cli,
        );
        builder.model_provider = Some(config.default_model_provider_id.clone());
        builder.cwd = home.path().to_path_buf();
        builder.cli_version = Some("test_version".to_string());
        let metadata = builder.build(config.default_model_provider_id.as_str());
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("state db upsert should succeed");

        store
            .archive_thread(ArchiveThreadParams { thread_id })
            .await
            .expect("archive thread");

        let archived_path = home
            .path()
            .join(ARCHIVED_SESSIONS_SUBDIR)
            .join(active_path.file_name().expect("file name"));
        let updated = runtime
            .get_thread(thread_id)
            .await
            .expect("state db read should succeed")
            .expect("thread metadata should exist");
        assert_eq!(updated.rollout_path, archived_path);
        assert!(updated.archived_at.is_some());
        assert_eq!(updated.recency_at, metadata.recency_at);
    }
}
