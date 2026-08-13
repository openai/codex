//! Local hard-delete support for persisted threads.
//!
//! Existing rollout files are deleted before this operation reports success. A rollout file that
//! vanishes after discovery counts as already deleted. The app-server deletes main state DB rows
//! after every associated rollout is removed; this module deletes local history projection rows.

use std::collections::HashMap;
use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use codex_rollout::ARCHIVED_SESSIONS_SUBDIR;
use codex_rollout::RolloutReferenceIndex;
use codex_rollout::SESSIONS_SUBDIR;
use codex_rollout::find_archived_thread_path_by_id_str;
use codex_rollout::find_thread_path_by_id_str;
use codex_rollout::remove_thread_name_entries;

use super::LocalThreadStore;
use super::helpers::scoped_rollout_path;
use super::helpers::validated_rollout_file_name;
use crate::DeleteThreadParams;
use crate::DeleteThreadsParams;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

struct ThreadRollouts {
    thread_id: codex_protocol::ThreadId,
    rollout_ids: HashSet<codex_protocol::ThreadId>,
    paths: Vec<PathBuf>,
}

impl ThreadRollouts {
    fn from_index(
        reference_index: &RolloutReferenceIndex,
        thread_id: codex_protocol::ThreadId,
    ) -> Self {
        let mut rollout_ids = HashSet::new();
        let paths = reference_index
            .rollouts_for_thread(thread_id)
            .map(|(rollout_id, path)| {
                rollout_ids.insert(rollout_id);
                path.to_path_buf()
            })
            .collect();
        Self {
            thread_id,
            rollout_ids,
            paths,
        }
    }

    fn add_path(&mut self, path: PathBuf) {
        if let Some(rollout_id) = codex_rollout::rollout_id_from_path(path.as_path()) {
            self.rollout_ids.insert(rollout_id);
        }
        if !self.paths.contains(&path) {
            self.paths.push(path);
        }
    }
}

pub(super) async fn delete_thread(
    store: &LocalThreadStore,
    params: DeleteThreadParams,
) -> ThreadStoreResult<()> {
    let thread_id = params.thread_id;
    let _lifecycle_guard = store.live_writer_locks.lock_lifecycle(thread_id).await;
    let _live_writer_guard = store.live_writer_locks.lock(thread_id).await;
    let reference_index = scan_reference_index(store).await?;
    let thread_rollouts = ThreadRollouts::from_index(&reference_index, thread_id);
    ensure_no_external_references(&reference_index, std::slice::from_ref(&thread_rollouts))?;
    let mut writer_guards = store.acquire_writer_locks(&[thread_id]).await?;
    delete_thread_after_reference_check(store, thread_rollouts, &mut writer_guards).await
}

pub(super) async fn delete_threads(
    store: &LocalThreadStore,
    params: DeleteThreadsParams,
) -> ThreadStoreResult<()> {
    let thread_ids = params.thread_ids;
    if thread_ids.is_empty() {
        return Ok(());
    }

    let mut lock_thread_ids = thread_ids.clone();
    lock_thread_ids.sort_unstable_by_key(ToString::to_string);
    lock_thread_ids.dedup();
    let mut _lifecycle_guards = Vec::with_capacity(lock_thread_ids.len());
    for thread_id in &lock_thread_ids {
        _lifecycle_guards.push(store.live_writer_locks.lock_lifecycle(*thread_id).await);
    }
    let mut _live_writer_guards = Vec::with_capacity(lock_thread_ids.len());
    for &thread_id in &lock_thread_ids {
        _live_writer_guards.push(store.live_writer_locks.lock(thread_id).await);
    }

    let reference_index = scan_reference_index(store).await?;
    let thread_rollouts = thread_ids
        .iter()
        .map(|thread_id| ThreadRollouts::from_index(&reference_index, *thread_id))
        .collect::<Vec<_>>();
    ensure_no_external_references(&reference_index, thread_rollouts.as_slice())?;

    let mut writer_guards = store.acquire_writer_locks(&lock_thread_ids).await?;
    for thread_rollouts in thread_rollouts {
        match delete_thread_after_reference_check(store, thread_rollouts, &mut writer_guards).await
        {
            Ok(()) | Err(ThreadStoreError::ThreadNotFound { .. }) => {}
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

fn ensure_no_external_references(
    reference_index: &RolloutReferenceIndex,
    thread_rollouts: &[ThreadRollouts],
) -> ThreadStoreResult<()> {
    let deletion_rollout_ids = thread_rollouts
        .iter()
        .flat_map(|thread_rollouts| thread_rollouts.rollout_ids.iter().copied())
        .collect::<HashSet<_>>();
    let mut internal_reference_counts = HashMap::new();
    for source_rollout_id in &deletion_rollout_ids {
        if let Some(history_base) = reference_index.history_base(*source_rollout_id)
            && history_base.thread_id != *source_rollout_id
            && deletion_rollout_ids.contains(&history_base.thread_id)
        {
            *internal_reference_counts
                .entry(history_base.thread_id)
                .or_default() += 1;
        }
    }
    for thread_rollouts in thread_rollouts {
        if thread_rollouts.rollout_ids.iter().any(|rollout_id| {
            let internal_reference_count = internal_reference_counts
                .get(rollout_id)
                .copied()
                .unwrap_or_default();
            reference_index.reference_count(*rollout_id) > internal_reference_count
        }) {
            return Err(referenced_thread_error(thread_rollouts.thread_id));
        }
    }
    Ok(())
}

async fn scan_reference_index(
    store: &LocalThreadStore,
) -> ThreadStoreResult<RolloutReferenceIndex> {
    RolloutReferenceIndex::scan(store.config.codex_home.as_path())
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to scan fork history references: {err}"),
        })
}

fn referenced_thread_error(thread_id: codex_protocol::ThreadId) -> ThreadStoreError {
    ThreadStoreError::InvalidRequest {
        message: format!("cannot delete thread {thread_id}: forked history still references it"),
    }
}

async fn delete_thread_after_reference_check(
    store: &LocalThreadStore,
    mut thread_rollouts: ThreadRollouts,
    writer_guards: &mut Vec<super::writer_lock::WriterLockGuard>,
) -> ThreadStoreResult<()> {
    let thread_id = thread_rollouts.thread_id;
    let thread_id_str = thread_id.to_string();
    let state_db_ctx = store.state_db().await;
    match find_thread_path_by_id_str(
        store.config.codex_home.as_path(),
        thread_id_str.as_str(),
        state_db_ctx.as_deref(),
    )
    .await
    {
        Ok(Some(path)) => thread_rollouts.add_path(path),
        Ok(None) => {}
        Err(err) => {
            return Err(ThreadStoreError::InvalidRequest {
                message: format!("failed to locate thread id {thread_id}: {err}"),
            });
        }
    }
    match find_archived_thread_path_by_id_str(
        store.config.codex_home.as_path(),
        thread_id_str.as_str(),
        state_db_ctx.as_deref(),
    )
    .await
    {
        Ok(Some(path)) => thread_rollouts.add_path(path),
        Ok(None) => {}
        Err(err) => {
            return Err(ThreadStoreError::InvalidRequest {
                message: format!("failed to locate archived thread id {thread_id}: {err}"),
            });
        }
    }
    thread_rollouts.rollout_ids.insert(thread_id);
    for rollout_id in thread_rollouts.rollout_ids {
        super::thread_history::delete_thread(store, rollout_id).await?;
    }

    // Drop the recorder before removing files, but retain its writer lock until cleanup finishes.
    if let Some(entry) = store.live_recorders.lock().await.remove(&thread_id) {
        writer_guards.push(entry.writer_lock);
    }
    let found_rollout_path = !thread_rollouts.paths.is_empty();
    for rollout_path in thread_rollouts.paths {
        delete_rollout_file(store, rollout_path.as_path())?;
    }
    remove_thread_name_entries(store.config.codex_home.as_path(), thread_id)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to delete thread name index entries for {thread_id}: {err}"),
        })?;

    if !found_rollout_path {
        return Err(ThreadStoreError::ThreadNotFound { thread_id });
    }

    Ok(())
}

fn delete_rollout_file(store: &LocalThreadStore, rollout_path: &Path) -> ThreadStoreResult<bool> {
    let plain_path = codex_rollout::plain_rollout_path(rollout_path);
    let compressed_path = plain_path.with_extension("jsonl.zst");
    let deleted_plain = delete_rollout_path(store, plain_path.as_path())?;
    let deleted_compressed = delete_rollout_path(store, compressed_path.as_path())?;
    Ok(deleted_plain || deleted_compressed)
}

fn delete_rollout_path(store: &LocalThreadStore, rollout_path: &Path) -> ThreadStoreResult<bool> {
    let canonical_rollout_path = scoped_rollout_path(
        store.config.codex_home.join(SESSIONS_SUBDIR),
        rollout_path,
        "sessions",
    )
    .or_else(|_| {
        scoped_rollout_path(
            store.config.codex_home.join(ARCHIVED_SESSIONS_SUBDIR),
            rollout_path,
            "archived sessions",
        )
    })
    .or_else(|err| match rollout_path.try_exists() {
        Ok(false) => Ok(rollout_path.to_path_buf()),
        Ok(true) | Err(_) => Err(err),
    })?;
    validated_rollout_file_name(&canonical_rollout_path, rollout_path)?;
    match std::fs::remove_file(&canonical_rollout_path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => Err(ThreadStoreError::Internal {
            message: format!(
                "failed to delete rollout file `{}`: {err}",
                canonical_rollout_path.display()
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::HistoryPosition;
    use codex_protocol::protocol::ThreadHistoryMode;
    use codex_protocol::protocol::ThreadMemoryMode;
    use codex_utils_absolute_path::test_support::PathExt;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;
    use crate::ResumeThreadParams;
    use crate::ThreadPersistenceMetadata;
    use crate::ThreadStore;
    use crate::local::LocalThreadStore;
    use crate::local::test_support::test_config;
    use crate::local::test_support::write_archived_session_file;
    use crate::local::test_support::write_session_file;
    use crate::local::test_support::write_session_file_with;
    use crate::local::test_support::write_session_file_with_history_mode;

    #[tokio::test]
    async fn delete_thread_removes_active_and_archived_rollouts() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let active_path =
            write_session_file(home.path(), "2025-01-03T12-00-00", Uuid::from_u128(301))
                .expect("session file");
        let compressed_path = active_path.with_extension("jsonl.zst");
        std::fs::write(&compressed_path, b"compressed sibling").expect("compressed sibling");
        let cases = [
            (Uuid::from_u128(301), active_path),
            (
                Uuid::from_u128(302),
                write_archived_session_file(
                    home.path(),
                    "2025-01-03T12-00-00",
                    Uuid::from_u128(302),
                )
                .expect("archived session file"),
            ),
        ];

        for (uuid, path) in cases {
            let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
            store
                .delete_thread(DeleteThreadParams { thread_id })
                .await
                .expect("delete thread");

            assert!(!path.exists());
        }
        assert!(!compressed_path.exists());
    }

    #[tokio::test]
    async fn delete_thread_rejects_referenced_paginated_history() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let source_uuid = Uuid::from_u128(303);
        let source_thread_id =
            ThreadId::from_string(&source_uuid.to_string()).expect("valid source thread id");
        let source_path = write_session_file_with_history_mode(
            home.path(),
            "2025-01-03T12-00-00",
            source_uuid,
            ThreadHistoryMode::Paginated,
        )
        .expect("source session file");
        let child_path = write_session_file_with(
            home.path(),
            home.path().join(ARCHIVED_SESSIONS_SUBDIR),
            "2025-01-03T12-00-01",
            Uuid::from_u128(304),
            "Archived user message",
            Some("test-provider"),
            ThreadHistoryMode::Paginated,
        )
        .expect("child session file");
        set_history_base(
            child_path.as_path(),
            HistoryPosition {
                thread_id: source_thread_id,
                end_ordinal_exclusive: 1,
                end_byte_offset: std::fs::metadata(source_path.as_path())
                    .expect("source rollout metadata")
                    .len(),
            },
        );

        let err = store
            .delete_thread(DeleteThreadParams {
                thread_id: source_thread_id,
            })
            .await
            .expect_err("referenced source should not be deleted");

        assert_eq!(
            err.to_string(),
            format!(
                "invalid thread-store request: cannot delete thread {source_thread_id}: forked history still references it"
            )
        );
        assert!(source_path.exists());
    }

    #[tokio::test]
    async fn delete_thread_rejects_reference_to_reverted_rollout() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let thread_uuid = Uuid::from_u128(320);
        let thread_id = ThreadId::from_string(&thread_uuid.to_string()).expect("thread id");
        let source_path = write_session_file_with_history_mode(
            home.path(),
            "2025-01-03T12-00-00",
            thread_uuid,
            ThreadHistoryMode::Paginated,
        )
        .expect("source session file");
        let replacement_uuid = Uuid::from_u128(321);
        let replacement_rollout_id =
            ThreadId::from_string(&replacement_uuid.to_string()).expect("replacement rollout id");
        let replacement_path = write_session_file_with_history_mode(
            home.path(),
            "2025-01-03T12-00-01",
            replacement_uuid,
            ThreadHistoryMode::Paginated,
        )
        .expect("replacement session file");
        set_thread_id(replacement_path.as_path(), thread_id);
        set_history_base(
            replacement_path.as_path(),
            HistoryPosition {
                thread_id,
                end_ordinal_exclusive: 1,
                end_byte_offset: std::fs::metadata(source_path.as_path())
                    .expect("source rollout metadata")
                    .len(),
            },
        );
        let child_path = write_session_file_with_history_mode(
            home.path(),
            "2025-01-03T12-00-02",
            Uuid::from_u128(322),
            ThreadHistoryMode::Paginated,
        )
        .expect("child session file");
        set_history_base(
            child_path.as_path(),
            HistoryPosition {
                thread_id: replacement_rollout_id,
                end_ordinal_exclusive: 1,
                end_byte_offset: std::fs::metadata(replacement_path.as_path())
                    .expect("replacement rollout metadata")
                    .len(),
            },
        );

        let err = store
            .delete_thread(DeleteThreadParams { thread_id })
            .await
            .expect_err("referenced replacement should not be deleted");

        assert_eq!(
            err.to_string(),
            format!(
                "invalid thread-store request: cannot delete thread {thread_id}: forked history still references it"
            )
        );
        assert!(source_path.exists());
        assert!(replacement_path.exists());
    }

    #[tokio::test]
    async fn delete_thread_ignores_unreadable_reference_metadata() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let source_uuid = Uuid::from_u128(305);
        let source_thread_id =
            ThreadId::from_string(&source_uuid.to_string()).expect("valid source thread id");
        let source_path = write_session_file(home.path(), "2025-01-03T12-00-00", source_uuid)
            .expect("source session file");
        let unreadable_path = source_path.with_file_name(format!(
            "rollout-2025-01-03T12-00-01-{}.jsonl",
            Uuid::from_u128(306)
        ));
        std::fs::write(unreadable_path, "{not json}\n").expect("unreadable rollout metadata");

        store
            .delete_thread(DeleteThreadParams {
                thread_id: source_thread_id,
            })
            .await
            .expect("unreadable metadata should not block delete");

        assert!(!source_path.exists());
    }

    #[tokio::test]
    async fn delete_threads_allows_internal_history_references() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let source_uuid = Uuid::from_u128(307);
        let source_thread_id =
            ThreadId::from_string(&source_uuid.to_string()).expect("valid source thread id");
        let source_path = write_session_file_with_history_mode(
            home.path(),
            "2025-01-03T12-00-00",
            source_uuid,
            ThreadHistoryMode::Paginated,
        )
        .expect("source session file");
        let child_uuid = Uuid::from_u128(308);
        let child_thread_id =
            ThreadId::from_string(&child_uuid.to_string()).expect("valid child thread id");
        let child_path = write_session_file_with_history_mode(
            home.path(),
            "2025-01-03T12-00-01",
            child_uuid,
            ThreadHistoryMode::Paginated,
        )
        .expect("child session file");
        set_history_base(
            child_path.as_path(),
            HistoryPosition {
                thread_id: source_thread_id,
                end_ordinal_exclusive: 1,
                end_byte_offset: std::fs::metadata(source_path.as_path())
                    .expect("source rollout metadata")
                    .len(),
            },
        );

        store
            .delete_threads(DeleteThreadsParams {
                thread_ids: vec![child_thread_id, source_thread_id],
            })
            .await
            .expect("internal references should not block batch delete");

        assert!(!source_path.exists());
        assert!(!child_path.exists());
    }

    #[tokio::test]
    async fn delete_threads_rejects_owned_descendants_before_deleting_anything() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let owner = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        for (parent_uuid, child_uuid, history_mode) in [
            (
                Uuid::from_u128(309),
                Uuid::from_u128(310),
                ThreadHistoryMode::Legacy,
            ),
            (
                Uuid::from_u128(312),
                Uuid::from_u128(313),
                ThreadHistoryMode::Paginated,
            ),
        ] {
            let parent_thread_id =
                ThreadId::from_string(&parent_uuid.to_string()).expect("valid parent thread id");
            let parent_path = write_session_file_with_history_mode(
                home.path(),
                "2025-01-03T12-00-00",
                parent_uuid,
                history_mode,
            )
            .expect("parent session file");
            let child_thread_id =
                ThreadId::from_string(&child_uuid.to_string()).expect("valid child thread id");
            let child_path = write_session_file_with_history_mode(
                home.path(),
                "2025-01-03T12-00-01",
                child_uuid,
                history_mode,
            )
            .expect("child session file");
            let _owner_guard = owner
                .writer_lock_coordinator
                .acquire(child_thread_id)
                .expect("acquire child writer lock");

            let error = store
                .delete_threads(DeleteThreadsParams {
                    thread_ids: vec![parent_thread_id, child_thread_id],
                })
                .await
                .expect_err("owned descendant should block deletion");

            assert!(matches!(error, ThreadStoreError::Conflict { .. }));
            assert!(parent_path.exists());
            assert!(child_path.exists());
        }
    }

    #[tokio::test]
    async fn delete_threads_rejects_owned_thread_before_rollout_materializes() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let owner = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let thread_id = ThreadId::default();
        let _owner_guard = owner
            .writer_lock_coordinator
            .acquire(thread_id)
            .expect("acquire writer lock before rollout exists");

        let error = store
            .delete_threads(DeleteThreadsParams {
                thread_ids: vec![thread_id],
            })
            .await
            .expect_err("owned thread should block deletion before rollout exists");

        assert!(matches!(error, ThreadStoreError::Conflict { .. }));
    }

    #[tokio::test]
    async fn delete_threads_removes_rollout_with_unreadable_metadata() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let uuid = Uuid::from_u128(311);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let rollout_path =
            write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");
        std::fs::write(&rollout_path, "{not json}\n").expect("damage rollout metadata");

        store
            .delete_threads(DeleteThreadsParams {
                thread_ids: vec![thread_id],
            })
            .await
            .expect("delete rollout with unreadable metadata");

        assert!(!rollout_path.exists());
    }

    #[tokio::test]
    async fn delete_rollout_file_treats_vanished_path_as_already_deleted() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let uuid = Uuid::from_u128(305);
        let path =
            write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");
        std::fs::remove_file(&path).expect("remove session file");

        assert!(!delete_rollout_file(&store, path.as_path()).expect("delete rollout"));
    }

    #[tokio::test]
    async fn delete_thread_without_state_db_preserves_materialized_thread_history() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let store = LocalThreadStore::new(config.clone(), /*state_db*/ None);
        let uuid = Uuid::from_u128(312);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let rollout_path = write_session_file_with_history_mode(
            home.path(),
            "2025-01-03T12-00-00",
            uuid,
            ThreadHistoryMode::Paginated,
        )
        .expect("session file");
        let pool = codex_state::open_thread_history_db(&config.sqlite)
            .await
            .expect("open existing thread history database");
        let thread_id_string = thread_id.to_string();
        sqlx::query(
            "INSERT INTO thread_turns (thread_id, turn_id, rollout_ordinal, status) VALUES (?, 'turn-1', 1, 'completed')",
        )
        .bind(thread_id_string.as_str())
        .execute(&pool)
        .await
        .expect("insert turn");
        sqlx::query(
            "INSERT INTO thread_items (thread_id, turn_id, item_id, rollout_ordinal, created_at_ms, item_json) VALUES (?, 'turn-1', 'item-1', 2, 1, '{}')",
        )
        .bind(thread_id_string.as_str())
        .execute(&pool)
        .await
        .expect("insert item");
        sqlx::query(
            "INSERT INTO thread_history_projection_state (thread_id, next_rollout_byte_offset, next_rollout_ordinal) VALUES (?, 3, 3)",
        )
        .bind(thread_id_string.as_str())
        .execute(&pool)
        .await
        .expect("insert projection state");

        let error = store
            .delete_thread(DeleteThreadParams { thread_id })
            .await
            .expect_err("projected history without a state database should prevent deletion");

        assert!(matches!(
            error,
            ThreadStoreError::Unsupported {
                operation: "paginated_history"
            }
        ));
        assert!(rollout_path.exists());
        let counts = sqlx::query_as::<_, (i64, i64, i64)>(
            r#"
SELECT
    (SELECT COUNT(*) FROM thread_turns WHERE thread_id = ?),
    (SELECT COUNT(*) FROM thread_items WHERE thread_id = ?),
    (SELECT COUNT(*) FROM thread_history_projection_state WHERE thread_id = ?)
            "#,
        )
        .bind(thread_id_string.as_str())
        .bind(thread_id_string.as_str())
        .bind(thread_id_string.as_str())
        .fetch_one(&pool)
        .await
        .expect("read preserved history rows");
        assert_eq!(counts, (1, 1, 1));
    }

    #[tokio::test]
    async fn delete_thread_removes_materialized_thread_history() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let state_db = codex_state::StateRuntime::init(
            config.sqlite.clone(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("initialize state database for materialized history");
        let store = LocalThreadStore::new(config, Some(state_db));
        let uuid = Uuid::from_u128(306);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let rollout_path = write_session_file_with_history_mode(
            home.path(),
            "2025-01-03T12-00-00",
            uuid,
            ThreadHistoryMode::Paginated,
        )
        .expect("session file");
        let pool = codex_state::open_thread_history_db(
            &codex_state::SqliteConfig::new_for_testing(home.path().abs()),
        )
        .await
        .expect("open thread history db");
        let thread_id_string = thread_id.to_string();
        sqlx::query(
            "INSERT INTO thread_turns (thread_id, turn_id, rollout_ordinal, status) VALUES (?, 'turn-1', 1, 'completed')",
        )
        .bind(thread_id_string.as_str())
        .execute(&pool)
        .await
        .expect("insert turn");
        sqlx::query(
            "INSERT INTO thread_items (thread_id, turn_id, item_id, rollout_ordinal, created_at_ms, item_json) VALUES (?, 'turn-1', 'item-1', 2, 1, '{}')",
        )
        .bind(thread_id_string.as_str())
        .execute(&pool)
        .await
        .expect("insert item");
        sqlx::query(
            "INSERT INTO thread_history_projection_state (thread_id, next_rollout_byte_offset, next_rollout_ordinal) VALUES (?, 3, 3)",
        )
        .bind(thread_id_string.as_str())
        .execute(&pool)
        .await
        .expect("insert projection state");

        store
            .resume_thread(ResumeThreadParams {
                thread_id,
                rollout_path: Some(rollout_path),
                history: None,
                include_archived: false,
                metadata: ThreadPersistenceMetadata {
                    cwd: Some(home.path().to_path_buf()),
                    model_provider: "test-provider".to_string(),
                    memory_mode: ThreadMemoryMode::Enabled,
                },
            })
            .await
            .expect("resume paginated writer before deletion");
        let lock_path = home
            .path()
            .join("thread-writer-locks")
            .join(format!("{thread_id}.lock"));
        assert!(lock_path.exists());

        store
            .delete_thread(DeleteThreadParams { thread_id })
            .await
            .expect("delete thread");
        assert!(!lock_path.exists());

        let counts = sqlx::query_as::<_, (i64, i64, i64)>(
            r#"
SELECT
    (SELECT COUNT(*) FROM thread_turns WHERE thread_id = ?),
    (SELECT COUNT(*) FROM thread_items WHERE thread_id = ?),
    (SELECT COUNT(*) FROM thread_history_projection_state WHERE thread_id = ?)
            "#,
        )
        .bind(thread_id_string.as_str())
        .bind(thread_id_string.as_str())
        .bind(thread_id_string.as_str())
        .fetch_one(&pool)
        .await
        .expect("read remaining history rows");
        assert_eq!(counts, (0, 0, 0));
    }

    #[tokio::test]
    async fn delete_thread_reports_missing_thread() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000304").expect("valid thread id");

        let err = store
            .delete_thread(DeleteThreadParams { thread_id })
            .await
            .expect_err("missing thread should fail");
        assert_eq!(
            err.to_string(),
            "thread 00000000-0000-0000-0000-000000000304 not found"
        );
    }

    fn set_history_base(path: &Path, history_base: HistoryPosition) {
        let mut session_meta: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(path)
                .expect("read session file")
                .lines()
                .next()
                .expect("session metadata"),
        )
        .expect("parse session metadata");
        session_meta["payload"]["history_base"] =
            serde_json::to_value(history_base).expect("serialize history base");
        std::fs::write(path, format!("{session_meta}\n")).expect("write session file");
    }

    fn set_thread_id(path: &Path, thread_id: ThreadId) {
        let mut session_meta: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(path)
                .expect("read session file")
                .lines()
                .next()
                .expect("session metadata"),
        )
        .expect("parse session metadata");
        session_meta["payload"]["id"] = serde_json::to_value(thread_id).expect("thread id");
        std::fs::write(path, format!("{session_meta}\n")).expect("write session file");
    }
}
