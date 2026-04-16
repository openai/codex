use codex_rollout::RolloutRecorder;
use codex_rollout::find_archived_thread_path_by_id_str;
use codex_rollout::find_thread_path_by_id_str;
use codex_rollout::read_thread_item_from_rollout;

use super::LocalThreadStore;
use super::helpers::stored_thread_from_rollout_item;
use crate::ReadThreadParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) async fn read_thread(
    store: &LocalThreadStore,
    params: ReadThreadParams,
) -> ThreadStoreResult<StoredThread> {
    let thread_id = params.thread_id;
    let path = if params.include_archived {
        match find_thread_path_by_id_str(store.config.codex_home.as_path(), &thread_id.to_string())
            .await
            .map_err(|err| ThreadStoreError::InvalidRequest {
                message: format!("failed to locate thread id {thread_id}: {err}"),
            })? {
            Some(path) => Some(path),
            None => find_archived_thread_path_by_id_str(
                store.config.codex_home.as_path(),
                &thread_id.to_string(),
            )
            .await
            .map_err(|err| ThreadStoreError::InvalidRequest {
                message: format!("failed to locate archived thread id {thread_id}: {err}"),
            })?,
        }
    } else {
        find_thread_path_by_id_str(store.config.codex_home.as_path(), &thread_id.to_string())
            .await
            .map_err(|err| ThreadStoreError::InvalidRequest {
                message: format!("failed to locate thread id {thread_id}: {err}"),
            })?
    }
    .ok_or_else(|| ThreadStoreError::InvalidRequest {
        message: format!("no rollout found for thread id {thread_id}"),
    })?;

    let item = read_thread_item_from_rollout(path.clone())
        .await
        .ok_or_else(|| ThreadStoreError::Internal {
            message: format!("failed to read thread {}", path.display()),
        })?;
    let archived = item.path.starts_with(
        store
            .config
            .codex_home
            .join(codex_rollout::ARCHIVED_SESSIONS_SUBDIR),
    );
    let mut thread =
        stored_thread_from_rollout_item(item, archived, store.config.model_provider_id.as_str())
            .ok_or_else(|| ThreadStoreError::Internal {
                message: format!("failed to read thread id from {}", path.display()),
            })?;
    if params.include_history {
        let (items, _, _) = RolloutRecorder::load_rollout_items(path.as_path())
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to load thread history {}: {err}", path.display()),
            })?;
        thread.history = Some(StoredThreadHistory { thread_id, items });
    }
    Ok(thread)
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
    use crate::local::test_support::write_session_file;

    #[tokio::test]
    async fn read_thread_returns_active_rollout_summary() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()));
        let uuid = Uuid::from_u128(205);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let active_path =
            write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");

        let thread = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: false,
                include_history: true,
            })
            .await
            .expect("read thread");

        assert_eq!(thread.thread_id, thread_id);
        assert_eq!(thread.rollout_path, Some(active_path));
        assert_eq!(thread.archived_at, None);
        assert_eq!(thread.preview, "Hello from user");
        assert_eq!(
            thread.history.expect("history should load").thread_id,
            thread_id
        );
    }

    #[tokio::test]
    async fn read_thread_fails_without_rollout() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()));
        let uuid = Uuid::from_u128(206);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");

        let err = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: false,
                include_history: false,
            })
            .await
            .expect_err("read should fail without rollout");

        let ThreadStoreError::InvalidRequest { message } = err else {
            panic!("expected invalid request error");
        };
        assert_eq!(
            message,
            format!("no rollout found for thread id {thread_id}")
        );
    }
}
