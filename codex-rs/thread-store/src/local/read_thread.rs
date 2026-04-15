use codex_rollout::RolloutRecorder;
use codex_rollout::find_archived_thread_path_by_id_str;
use codex_rollout::find_thread_name_by_id;
use codex_rollout::find_thread_path_by_id_str;
use codex_rollout::read_session_meta_line;
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
    thread.forked_from_id = read_session_meta_line(path.as_path())
        .await
        .ok()
        .and_then(|meta_line| meta_line.meta.forked_from_id);
    let mut found_sqlite_title = false;
    if let Ok(runtime) = codex_state::StateRuntime::init(
        store.config.sqlite_home.clone(),
        store.config.model_provider_id.clone(),
    )
    .await
        && let Ok(Some(metadata)) = runtime.get_thread(thread.thread_id).await
    {
        if let Some(title) = distinct_title(&metadata) {
            found_sqlite_title = true;
            set_thread_name_from_title(&mut thread, title);
        }
        thread.git_info = if metadata.git_sha.is_none()
            && metadata.git_branch.is_none()
            && metadata.git_origin_url.is_none()
        {
            None
        } else {
            Some(codex_protocol::protocol::GitInfo {
                commit_hash: metadata
                    .git_sha
                    .as_deref()
                    .map(codex_git_utils::GitSha::new),
                branch: metadata.git_branch,
                repository_url: metadata.git_origin_url,
            })
        };
    }
    if !found_sqlite_title
        && let Ok(Some(title)) =
            find_thread_name_by_id(store.config.codex_home.as_path(), &thread_id).await
    {
        set_thread_name_from_title(&mut thread, title);
    }
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

fn distinct_title(metadata: &codex_state::ThreadMetadata) -> Option<String> {
    let title = metadata.title.trim();
    if title.is_empty() || metadata.first_user_message.as_deref().map(str::trim) == Some(title) {
        None
    } else {
        Some(title.to_string())
    }
}

fn set_thread_name_from_title(thread: &mut StoredThread, title: String) {
    if title.trim().is_empty() || thread.preview.trim() == title.trim() {
        return;
    }
    thread.name = Some(title);
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::SessionSource;
    use codex_state::ThreadMetadataBuilder;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;
    use crate::ThreadStore;
    use crate::local::LocalThreadStore;
    use crate::local::test_support::test_config;
    use crate::local::test_support::write_archived_session_file;
    use crate::local::test_support::write_session_file;
    use crate::local::test_support::write_session_file_with_fork;

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
        let history = thread.history.expect("history should load");
        assert_eq!(history.thread_id, thread_id);
        assert_eq!(history.items.len(), 2);
    }

    #[tokio::test]
    async fn read_thread_returns_archived_rollout_when_requested() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()));
        let uuid = Uuid::from_u128(207);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let archived_path = write_archived_session_file(home.path(), "2025-01-03T12-00-00", uuid)
            .expect("archived session file");

        let active_only_err = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: false,
                include_history: false,
            })
            .await
            .expect_err("active-only read should fail for archived rollout");
        let ThreadStoreError::InvalidRequest { message } = active_only_err else {
            panic!("expected invalid request error");
        };
        assert_eq!(
            message,
            format!("no rollout found for thread id {thread_id}")
        );

        let thread = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: true,
                include_history: false,
            })
            .await
            .expect("read archived thread");

        assert_eq!(thread.thread_id, thread_id);
        assert_eq!(thread.rollout_path, Some(archived_path));
        assert!(thread.archived_at.is_some());
        assert_eq!(thread.preview, "Archived user message");
        assert!(thread.history.is_none());
    }

    #[tokio::test]
    async fn read_thread_prefers_active_rollout_over_archived() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()));
        let uuid = Uuid::from_u128(208);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let active_path =
            write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");
        write_archived_session_file(home.path(), "2025-01-03T12-00-00", uuid)
            .expect("archived session file");

        let thread = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: true,
                include_history: false,
            })
            .await
            .expect("read thread");

        assert_eq!(thread.rollout_path, Some(active_path));
        assert_eq!(thread.archived_at, None);
        assert_eq!(thread.preview, "Hello from user");
    }

    #[tokio::test]
    async fn read_thread_returns_forked_from_id() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()));
        let uuid = Uuid::from_u128(209);
        let parent_uuid = Uuid::from_u128(210);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let parent_thread_id =
            ThreadId::from_string(&parent_uuid.to_string()).expect("valid parent thread id");
        write_session_file_with_fork(
            home.path(),
            home.path().join("sessions/2025/01/03"),
            "2025-01-03T12-00-00",
            uuid,
            "Forked user message",
            Some("test-provider"),
            Some(parent_uuid),
        )
        .expect("forked session file");

        let thread = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: false,
                include_history: false,
            })
            .await
            .expect("read thread");

        assert_eq!(thread.forked_from_id, Some(parent_thread_id));
    }

    #[tokio::test]
    async fn read_thread_applies_sqlite_git_metadata() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let store = LocalThreadStore::new(config.clone());
        let uuid = Uuid::from_u128(211);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let rollout_path =
            write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");
        let runtime = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let mut builder =
            ThreadMetadataBuilder::new(thread_id, rollout_path, Utc::now(), SessionSource::Cli);
        builder.model_provider = Some(config.model_provider_id.clone());
        builder.cwd = home.path().to_path_buf();
        builder.cli_version = Some("test_version".to_string());
        let mut metadata = builder.build(config.model_provider_id.as_str());
        metadata.git_sha = Some("abc123".to_string());
        metadata.git_branch = Some("feature/sqlite".to_string());
        metadata.git_origin_url = Some("git@example.com:openai/codex.git".to_string());
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("state db upsert should succeed");

        let thread = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: false,
                include_history: false,
            })
            .await
            .expect("read thread");

        let git_info = thread.git_info.expect("git info should be applied");
        assert_eq!(
            git_info.commit_hash.map(|sha| sha.0),
            Some("abc123".to_string())
        );
        assert_eq!(git_info.branch, Some("feature/sqlite".to_string()));
        assert_eq!(
            git_info.repository_url,
            Some("git@example.com:openai/codex.git".to_string())
        );
    }

    #[tokio::test]
    async fn read_thread_applies_sqlite_thread_name() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let store = LocalThreadStore::new(config.clone());
        let uuid = Uuid::from_u128(212);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let rollout_path =
            write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");
        let runtime = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let mut builder =
            ThreadMetadataBuilder::new(thread_id, rollout_path, Utc::now(), SessionSource::Cli);
        builder.model_provider = Some(config.model_provider_id.clone());
        builder.cwd = home.path().to_path_buf();
        builder.cli_version = Some("test_version".to_string());
        let mut metadata = builder.build(config.model_provider_id.as_str());
        metadata.title = "Saved title".to_string();
        metadata.first_user_message = Some("Hello from user".to_string());
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("state db upsert should succeed");

        let thread = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: false,
                include_history: false,
            })
            .await
            .expect("read thread");

        assert_eq!(thread.name, Some("Saved title".to_string()));
    }

    #[tokio::test]
    async fn read_thread_uses_legacy_thread_name_when_sqlite_title_is_missing() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()));
        let uuid = Uuid::from_u128(213);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        write_session_file(home.path(), "2025-01-03T12-00-00", uuid).expect("session file");
        codex_rollout::append_thread_name(home.path(), thread_id, "Legacy title")
            .await
            .expect("append legacy thread name");

        let thread = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: false,
                include_history: false,
            })
            .await
            .expect("read thread");

        assert_eq!(thread.name, Some("Legacy title".to_string()));
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
