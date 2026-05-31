//! Local hard-delete support for persisted threads.
//!
//! Existing rollout files are deleted before this operation reports success. Missing rollout files
//! count as already deleted; SQLite and compatibility metadata cleanup is best effort after rollout
//! deletion succeeds.

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
    let thread_id_str = thread_id.to_string();
    let state_db_ctx = store.state_db().await;
    let mut rollout_paths = Vec::new();
    let state_thread_exists = if let Some(ctx) = state_db_ctx.as_ref() {
        match ctx.get_thread(thread_id).await {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(err) => {
                warn!("failed to check thread metadata for {thread_id}: {err}");
                false
            }
        }
    } else {
        false
    };

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

    store.live_recorders.lock().await.remove(&thread_id);

    let mut deleted_rollout_file = false;
    for rollout_path in rollout_paths {
        deleted_rollout_file |= delete_rollout_file(store, rollout_path.as_path(), thread_id)?;
    }

    let deleted_state_rows = if let Some(ctx) = state_db_ctx.as_ref() {
        match ctx.delete_thread(thread_id).await {
            Ok(rows) => rows,
            Err(err) => {
                warn!("failed to delete thread metadata for {thread_id}: {err}");
                0
            }
        }
    } else {
        0
    };

    if !deleted_rollout_file && !state_thread_exists && deleted_state_rows == 0 {
        return Err(ThreadStoreError::ThreadNotFound { thread_id });
    }

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
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::SessionSource;
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
        let cases = [
            (
                Uuid::from_u128(301),
                write_session_file(home.path(), "2025-01-03T12-00-00", Uuid::from_u128(301))
                    .expect("session file"),
            ),
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
    }

    #[tokio::test]
    async fn delete_thread_treats_missing_rollout_as_already_deleted_when_sqlite_row_exists() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let runtime = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config.clone(), Some(runtime.clone()));
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000303").expect("valid thread id");
        let mut builder = codex_state::ThreadMetadataBuilder::new(
            thread_id,
            home.path().join("sessions/missing-rollout.jsonl"),
            Utc::now(),
            SessionSource::Cli,
        );
        builder.cwd = home.path().to_path_buf();
        let metadata = builder.build(config.default_model_provider_id.as_str());
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("state db upsert should succeed");

        store
            .delete_thread(DeleteThreadParams { thread_id })
            .await
            .expect("delete thread");

        assert_eq!(
            runtime
                .get_thread(thread_id)
                .await
                .expect("sqlite metadata read"),
            None
        );
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
