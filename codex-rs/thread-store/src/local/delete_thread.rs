//! Local hard-delete support for persisted threads.
//!
//! Existing rollout files are deleted before this operation reports success. A rollout file that
//! vanishes after discovery counts as already deleted. SQLite cleanup happens at the app-server
//! layer after every associated rollout has been removed so failed deletes can be retried.

use std::io::ErrorKind;
use std::path::Path;

use codex_rollout::ARCHIVED_SESSIONS_SUBDIR;
use codex_rollout::SESSIONS_SUBDIR;
use codex_rollout::find_archived_thread_path_by_id_str;
use codex_rollout::find_thread_path_by_id_str;
use codex_rollout::remove_thread_name_entries;

use super::LocalThreadStore;
use super::helpers::matching_rollout_file_name;
use super::helpers::scoped_rollout_path;
use crate::DeleteThreadParams;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) async fn delete_thread(
    store: &LocalThreadStore,
    params: DeleteThreadParams,
) -> ThreadStoreResult<()> {
    let thread_id = params.thread_id;
    super::live_writer::close_thread_for_delete(store, thread_id).await?;
    let _segment_storage_guard = store.segment_storage_lock.clone().lock_owned().await;
    let thread_id_str = thread_id.to_string();
    let state_db_ctx = store.state_db().await;
    let mut rollout_paths = Vec::new();

    match find_thread_path_by_id_str(
        store.config.codex_home.as_path(),
        thread_id_str.as_str(),
        state_db_ctx.as_deref(),
    )
    .await
    {
        Ok(Some(path)) => rollout_paths.push(path),
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
        Ok(Some(path)) => {
            if !rollout_paths.contains(&path) {
                rollout_paths.push(path);
            }
        }
        Ok(None) => {}
        Err(err) => {
            return Err(ThreadStoreError::InvalidRequest {
                message: format!("failed to locate archived thread id {thread_id}: {err}"),
            });
        }
    }

    let found_rollout_path = !rollout_paths.is_empty();
    let found_segment_tree = store
        .config
        .codex_home
        .join(codex_rollout::ROTATED_ROLLOUT_SEGMENTS_SUBDIR)
        .join(thread_id.to_string())
        .try_exists()
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to inspect rollout segments for thread {thread_id}: {err}"),
        })?;
    for rollout_path in rollout_paths {
        delete_rollout_file(store, rollout_path.as_path(), thread_id)?;
    }
    remove_thread_name_entries(store.config.codex_home.as_path(), thread_id)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to delete thread name index entries for {thread_id}: {err}"),
        })?;

    if !found_rollout_path && !found_segment_tree {
        return Err(ThreadStoreError::ThreadNotFound { thread_id });
    }

    super::segment_gc::collect_unreferenced_segments(store.config.codex_home.as_path())
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!(
                "failed to collect rollout segments after deleting {thread_id}: {err}"
            ),
        })?;

    Ok(())
}

fn delete_rollout_file(
    store: &LocalThreadStore,
    rollout_path: &Path,
    thread_id: codex_protocol::ThreadId,
) -> ThreadStoreResult<bool> {
    let plain_path = codex_rollout::plain_rollout_path(rollout_path);
    let compressed_path = plain_path.with_extension("jsonl.zst");
    let deleted_plain = delete_rollout_path(store, plain_path.as_path(), thread_id)?;
    let deleted_compressed = delete_rollout_path(store, compressed_path.as_path(), thread_id)?;
    Ok(deleted_plain || deleted_compressed)
}

fn delete_rollout_path(
    store: &LocalThreadStore,
    rollout_path: &Path,
    thread_id: codex_protocol::ThreadId,
) -> ThreadStoreResult<bool> {
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
    matching_rollout_file_name(&canonical_rollout_path, thread_id, rollout_path)?;
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
    async fn delete_rollout_file_treats_vanished_path_as_already_deleted() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let uuid = Uuid::from_u128(305);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let path =
            write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");
        std::fs::remove_file(&path).expect("remove session file");

        assert!(!delete_rollout_file(&store, path.as_path(), thread_id).expect("delete rollout"));
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
}
