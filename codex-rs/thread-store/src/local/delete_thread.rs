//! Local hard-delete support for persisted threads.
//!
//! Deleting the state DB row is the commit point when SQLite is available; rollout files and
//! compatibility artifacts are removed best effort after that point.

use std::io::ErrorKind;
use std::path::Path;

use codex_rollout::ARCHIVED_SESSIONS_SUBDIR;
use codex_rollout::SESSIONS_SUBDIR;
use codex_rollout::find_archived_thread_path_by_id_str;
use codex_rollout::find_thread_path_by_id_str;
use tracing::warn;

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
    let state_db_ctx = store.state_db().await;
    let mut rollout_paths = Vec::new();
    let mut path_lookup_errors = Vec::new();

    match find_thread_path_by_id_str(
        store.config.codex_home.as_path(),
        &thread_id.to_string(),
        state_db_ctx.as_deref(),
    )
    .await
    {
        Ok(Some(path)) => rollout_paths.push(path),
        Ok(None) => {}
        Err(err) => {
            path_lookup_errors.push(format!("failed to locate thread id {thread_id}: {err}"))
        }
    }

    match find_archived_thread_path_by_id_str(
        store.config.codex_home.as_path(),
        &thread_id.to_string(),
        state_db_ctx.as_deref(),
    )
    .await
    {
        Ok(Some(path)) => {
            if !rollout_paths.iter().any(|existing| existing == &path) {
                rollout_paths.push(path);
            }
        }
        Ok(None) => {}
        Err(err) => {
            path_lookup_errors.push(format!(
                "failed to locate archived thread id {thread_id}: {err}"
            ));
        }
    }

    let deleted_state_rows = if let Some(ctx) = state_db_ctx.as_ref() {
        ctx.delete_thread(thread_id)
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to delete thread metadata for {thread_id}: {err}"),
            })?
    } else {
        0
    };
    if deleted_state_rows > 0 {
        for message in &path_lookup_errors {
            warn!("{message}");
        }
    }

    let mut deleted_rollout_file = false;
    for rollout_path in rollout_paths {
        match delete_rollout_file(store, rollout_path.as_path(), thread_id) {
            Ok(deleted) => deleted_rollout_file |= deleted,
            Err(err) if deleted_state_rows > 0 => {
                warn!("failed to delete rollout file for thread {thread_id}: {err}");
            }
            Err(err) => return Err(err),
        }
    }

    if deleted_state_rows == 0 && !deleted_rollout_file {
        if let Some(message) = path_lookup_errors.into_iter().next() {
            return Err(ThreadStoreError::InvalidRequest { message });
        }
        return Err(ThreadStoreError::ThreadNotFound { thread_id });
    }

    store.live_recorders.lock().await.remove(&thread_id);

    Ok(())
}

fn delete_rollout_file(
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
    use crate::ReadThreadParams;
    use crate::ThreadStore;
    use crate::local::LocalThreadStore;
    use crate::local::test_support::test_config;
    use crate::local::test_support::write_archived_session_file;
    use crate::local::test_support::write_session_file;

    #[tokio::test]
    async fn delete_thread_removes_active_rollout() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let uuid = Uuid::from_u128(301);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let active_path =
            write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");

        store
            .delete_thread(DeleteThreadParams { thread_id })
            .await
            .expect("delete thread");

        assert!(!active_path.exists());
        assert!(
            store
                .read_thread(ReadThreadParams {
                    thread_id,
                    include_archived: true,
                    include_history: false,
                })
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn delete_thread_removes_archived_rollout() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let uuid = Uuid::from_u128(302);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let archived_path = write_archived_session_file(home.path(), "2025-01-03T12-00-00", uuid)
            .expect("archived session file");

        store
            .delete_thread(DeleteThreadParams { thread_id })
            .await
            .expect("delete thread");

        assert!(!archived_path.exists());
    }

    #[tokio::test]
    async fn delete_thread_reports_missing_thread() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000303").expect("valid thread id");

        let err = store
            .delete_thread(DeleteThreadParams { thread_id })
            .await
            .expect_err("missing thread should fail");
        assert_eq!(
            err.to_string(),
            "thread 00000000-0000-0000-0000-000000000303 not found"
        );
    }
}
