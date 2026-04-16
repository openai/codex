use serde::de::DeserializeOwned;

use chrono::DateTime;
use chrono::Utc;
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
    // Local store resolution is rollout-first so summary extraction stays aligned
    // with list/read behavior for normal persisted sessions. SQLite is a fallback
    // for metadata rows that refer to rollouts not discoverable under codex_home,
    // such as explicit-path resumes or legacy state.
    let Some(path) = resolve_rollout_path(store, &params).await? else {
        return read_thread_from_sqlite_fallback(store, params).await;
    };

    read_thread_from_rollout_path(store, params, path).await
}

async fn resolve_rollout_path(
    store: &LocalThreadStore,
    params: &ReadThreadParams,
) -> ThreadStoreResult<Option<std::path::PathBuf>> {
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
    };
    Ok(path)
}

async fn read_thread_from_rollout_path(
    store: &LocalThreadStore,
    params: ReadThreadParams,
    path: std::path::PathBuf,
) -> ThreadStoreResult<StoredThread> {
    let thread_id = params.thread_id;
    let archived = path.starts_with(
        store
            .config
            .codex_home
            .join(codex_rollout::ARCHIVED_SESSIONS_SUBDIR),
    );
    let Some(item) = read_thread_item_from_rollout(path.clone()).await else {
        // Some materialized sessions, such as standalone shell-command turns, have
        // valid persisted history and SQLite metadata before they have a user-message
        // preview that the rollout summary reader can use.
        let mut thread =
            if let Ok(Some(mut metadata)) = read_sqlite_metadata(store, thread_id).await {
                metadata.rollout_path = path.clone();
                stored_thread_from_sqlite_metadata(store, metadata).await
            } else {
                stored_thread_from_session_meta(store, path.clone(), archived).await?
            };
        if params.include_history {
            let Some(path) = thread.rollout_path.clone() else {
                return Err(ThreadStoreError::Internal {
                    message: format!("failed to load thread history for thread {thread_id}"),
                });
            };
            let items = load_history_items(&path).await?;
            thread.history = Some(StoredThreadHistory { thread_id, items });
        }
        return Ok(thread);
    };
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
    if let Ok(Some(metadata)) = read_sqlite_metadata(store, thread.thread_id).await {
        if let Some(title) = distinct_title(&metadata) {
            found_sqlite_title = true;
            set_thread_name_from_title(&mut thread, title);
        }
        if let Some(git_info) = git_info_from_metadata(&metadata) {
            thread.git_info = Some(git_info);
        }
    }
    if !found_sqlite_title
        && let Ok(Some(title)) =
            find_thread_name_by_id(store.config.codex_home.as_path(), &thread_id).await
    {
        set_thread_name_from_title(&mut thread, title);
    }
    if params.include_history {
        let items = load_history_items(&path).await?;
        thread.history = Some(StoredThreadHistory { thread_id, items });
    }
    Ok(thread)
}

async fn read_thread_from_sqlite_fallback(
    store: &LocalThreadStore,
    params: ReadThreadParams,
) -> ThreadStoreResult<StoredThread> {
    let thread_id = params.thread_id;
    let metadata = read_sqlite_metadata(store, thread_id).await?;
    let Some(metadata) = metadata
        .filter(|metadata| params.include_archived || !metadata_is_archived(store, metadata))
    else {
        return Err(ThreadStoreError::InvalidRequest {
            message: format!("no rollout found for thread id {thread_id}"),
        });
    };

    let mut thread = stored_thread_from_sqlite_metadata(store, metadata).await;
    if params.include_history {
        let Some(path) = thread.rollout_path.clone() else {
            return Err(ThreadStoreError::Internal {
                message: format!("failed to load thread history for thread {thread_id}"),
            });
        };
        let items = load_history_items(&path).await?;
        thread.history = Some(StoredThreadHistory { thread_id, items });
    }
    Ok(thread)
}

async fn read_sqlite_metadata(
    store: &LocalThreadStore,
    thread_id: codex_protocol::ThreadId,
) -> ThreadStoreResult<Option<codex_state::ThreadMetadata>> {
    let runtime = codex_state::StateRuntime::init(
        store.config.sqlite_home.clone(),
        store.config.model_provider_id.clone(),
    )
    .await
    .map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to initialize SQLite state runtime: {err}"),
    })?;
    runtime
        .get_thread(thread_id)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to read SQLite metadata for thread {thread_id}: {err}"),
        })
}

fn metadata_is_archived(store: &LocalThreadStore, metadata: &codex_state::ThreadMetadata) -> bool {
    metadata.archived_at.is_some()
        || metadata.rollout_path.starts_with(
            store
                .config
                .codex_home
                .join(codex_rollout::ARCHIVED_SESSIONS_SUBDIR),
        )
}

async fn stored_thread_from_session_meta(
    store: &LocalThreadStore,
    path: std::path::PathBuf,
    archived: bool,
) -> ThreadStoreResult<StoredThread> {
    let session_meta_line = read_session_meta_line(path.as_path())
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to read thread {}: {err}", path.display()),
        })?;
    let created_at =
        parse_rfc3339_datetime(session_meta_line.meta.timestamp.as_str()).unwrap_or_else(Utc::now);
    let updated_at = read_rollout_updated_at(path.as_path()).unwrap_or(created_at);
    let archived_at = archived.then_some(updated_at);
    let model_provider = session_meta_line
        .meta
        .model_provider
        .clone()
        .filter(|provider| !provider.is_empty())
        .unwrap_or_else(|| store.config.model_provider_id.clone());

    Ok(StoredThread {
        thread_id: session_meta_line.meta.id,
        rollout_path: Some(path),
        forked_from_id: session_meta_line.meta.forked_from_id,
        preview: String::new(),
        name: None,
        model_provider,
        model: None,
        reasoning_effort: None,
        created_at,
        updated_at,
        archived_at,
        cwd: session_meta_line.meta.cwd,
        cli_version: session_meta_line.meta.cli_version,
        source: session_meta_line.meta.source,
        agent_nickname: session_meta_line.meta.agent_nickname,
        agent_role: session_meta_line.meta.agent_role,
        agent_path: session_meta_line.meta.agent_path,
        git_info: session_meta_line.git,
        approval_mode: codex_protocol::protocol::AskForApproval::OnRequest,
        sandbox_policy: codex_protocol::protocol::SandboxPolicy::new_read_only_policy(),
        token_usage: None,
        first_user_message: None,
        history: None,
    })
}

async fn stored_thread_from_sqlite_metadata(
    store: &LocalThreadStore,
    metadata: codex_state::ThreadMetadata,
) -> StoredThread {
    let mut thread = StoredThread {
        thread_id: metadata.id,
        rollout_path: Some(metadata.rollout_path.clone()),
        forked_from_id: read_session_meta_line(metadata.rollout_path.as_path())
            .await
            .ok()
            .and_then(|meta_line| meta_line.meta.forked_from_id),
        preview: metadata.first_user_message.clone().unwrap_or_default(),
        name: None,
        model_provider: if metadata.model_provider.is_empty() {
            store.config.model_provider_id.clone()
        } else {
            metadata.model_provider.clone()
        },
        model: metadata.model.clone(),
        reasoning_effort: metadata.reasoning_effort,
        created_at: metadata.created_at,
        updated_at: metadata.updated_at,
        archived_at: metadata.archived_at,
        cwd: metadata.cwd.clone(),
        cli_version: metadata.cli_version.clone(),
        source: parse_metadata_enum(&metadata.source)
            .unwrap_or(codex_protocol::protocol::SessionSource::Unknown),
        agent_nickname: metadata.agent_nickname.clone(),
        agent_role: metadata.agent_role.clone(),
        agent_path: metadata.agent_path.clone(),
        git_info: git_info_from_metadata(&metadata),
        approval_mode: parse_metadata_enum(&metadata.approval_mode)
            .unwrap_or(codex_protocol::protocol::AskForApproval::OnRequest),
        sandbox_policy: parse_sandbox_policy(metadata.sandbox_policy.as_str()),
        token_usage: (metadata.tokens_used > 0).then(|| codex_protocol::protocol::TokenUsage {
            total_tokens: metadata.tokens_used,
            ..Default::default()
        }),
        first_user_message: metadata.first_user_message.clone(),
        history: None,
    };

    if let Some(title) = distinct_title(&metadata) {
        set_thread_name_from_title(&mut thread, title);
    } else if let Ok(Some(title)) =
        find_thread_name_by_id(store.config.codex_home.as_path(), &metadata.id).await
    {
        set_thread_name_from_title(&mut thread, title);
    }
    thread
}

async fn load_history_items(
    path: &std::path::Path,
) -> ThreadStoreResult<Vec<codex_protocol::protocol::RolloutItem>> {
    let (items, _, _) = RolloutRecorder::load_rollout_items(path)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to load thread history {}: {err}", path.display()),
        })?;
    Ok(items)
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

fn git_info_from_metadata(
    metadata: &codex_state::ThreadMetadata,
) -> Option<codex_protocol::protocol::GitInfo> {
    if metadata.git_sha.is_none()
        && metadata.git_branch.is_none()
        && metadata.git_origin_url.is_none()
    {
        return None;
    }
    Some(codex_protocol::protocol::GitInfo {
        commit_hash: metadata
            .git_sha
            .as_deref()
            .map(codex_git_utils::GitSha::new),
        branch: metadata.git_branch.clone(),
        repository_url: metadata.git_origin_url.clone(),
    })
}

fn parse_metadata_enum<T: DeserializeOwned>(value: &str) -> Option<T> {
    serde_json::from_str(value)
        .or_else(|_| serde_json::from_value(serde_json::Value::String(value.to_string())))
        .ok()
}

fn parse_rfc3339_datetime(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn read_rollout_updated_at(path: &std::path::Path) -> Option<DateTime<Utc>> {
    std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .map(DateTime::<Utc>::from)
}

fn parse_sandbox_policy(value: &str) -> codex_protocol::protocol::SandboxPolicy {
    parse_metadata_enum(value)
        .or_else(|| match value.trim() {
            "danger-full-access" => Some(codex_protocol::protocol::SandboxPolicy::DangerFullAccess),
            "read-only" => Some(codex_protocol::protocol::SandboxPolicy::new_read_only_policy()),
            "workspace-write" => {
                Some(codex_protocol::protocol::SandboxPolicy::new_workspace_write_policy())
            }
            _ => None,
        })
        .unwrap_or_else(codex_protocol::protocol::SandboxPolicy::new_read_only_policy)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

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
    async fn read_thread_keeps_rollout_git_metadata_when_sqlite_git_metadata_is_empty() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let store = LocalThreadStore::new(config.clone());
        let uuid = Uuid::from_u128(219);
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
        let metadata = builder.build(config.model_provider_id.as_str());
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

        let git_info = thread.git_info.expect("rollout git info should be kept");
        assert_eq!(
            git_info.commit_hash.map(|sha| sha.0),
            Some("abcdef".to_string())
        );
        assert_eq!(git_info.branch.as_deref(), Some("main"));
        assert_eq!(
            git_info.repository_url.as_deref(),
            Some("https://example.com/repo.git")
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
    async fn read_thread_uses_sqlite_metadata_for_rollout_without_user_preview() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let store = LocalThreadStore::new(config.clone());
        let uuid = Uuid::from_u128(217);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let day_dir = home.path().join("sessions/2025/01/03");
        std::fs::create_dir_all(&day_dir).expect("sessions dir");
        let rollout_path = day_dir.join(format!("rollout-2025-01-03T12-00-00-{uuid}.jsonl"));
        let mut file = std::fs::File::create(&rollout_path).expect("session file");
        let meta = serde_json::json!({
            "timestamp": "2025-01-03T12-00-00",
            "type": "session_meta",
            "payload": {
                "id": uuid,
                "timestamp": "2025-01-03T12-00-00",
                "cwd": home.path(),
                "originator": "test_originator",
                "cli_version": "test_version",
                "source": "cli",
                "model_provider": "rollout-provider"
            },
        });
        writeln!(file, "{meta}").expect("write session meta");

        let runtime = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let mut builder = ThreadMetadataBuilder::new(
            thread_id,
            rollout_path.clone(),
            Utc::now(),
            SessionSource::Cli,
        );
        builder.model_provider = Some("sqlite-provider".to_string());
        builder.cwd = home.path().join("workspace");
        builder.cli_version = Some("sqlite-cli".to_string());
        let mut metadata = builder.build(config.model_provider_id.as_str());
        metadata.title = "Command-only thread".to_string();
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("state db upsert should succeed");

        let thread = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: false,
                include_history: true,
            })
            .await
            .expect("read thread");

        assert_eq!(thread.thread_id, thread_id);
        assert_eq!(thread.rollout_path, Some(rollout_path));
        assert_eq!(thread.preview, "");
        assert_eq!(thread.name.as_deref(), Some("Command-only thread"));
        assert_eq!(thread.model_provider, "sqlite-provider");
        assert_eq!(thread.cwd, home.path().join("workspace"));
        assert_eq!(thread.cli_version, "sqlite-cli");
        let history = thread.history.expect("history should load");
        assert_eq!(history.thread_id, thread_id);
        assert_eq!(history.items.len(), 1);
    }

    #[tokio::test]
    async fn read_thread_uses_session_meta_for_rollout_without_user_preview_or_sqlite_metadata() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()));
        let uuid = Uuid::from_u128(218);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let day_dir = home.path().join("sessions/2025/01/03");
        std::fs::create_dir_all(&day_dir).expect("sessions dir");
        let rollout_path = day_dir.join(format!("rollout-2025-01-03T12-00-00-{uuid}.jsonl"));
        let mut file = std::fs::File::create(&rollout_path).expect("session file");
        let meta = serde_json::json!({
            "timestamp": "2025-01-03T12:00:00Z",
            "type": "session_meta",
            "payload": {
                "id": uuid,
                "timestamp": "2025-01-03T12:00:00Z",
                "cwd": home.path(),
                "originator": "test_originator",
                "cli_version": "test_version",
                "source": "cli",
                "model_provider": "rollout-provider",
                "git": {
                    "commit_hash": "abcdef",
                    "branch": "main",
                    "repository_url": "https://example.com/repo.git"
                }
            },
        });
        writeln!(file, "{meta}").expect("write session meta");

        let thread = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: false,
                include_history: true,
            })
            .await
            .expect("read thread");

        assert_eq!(thread.thread_id, thread_id);
        assert_eq!(thread.rollout_path, Some(rollout_path));
        assert_eq!(thread.preview, "");
        assert_eq!(thread.name, None);
        assert_eq!(thread.model_provider, "rollout-provider");
        assert_eq!(
            thread.created_at,
            parse_rfc3339_datetime("2025-01-03T12:00:00Z").unwrap()
        );
        assert!(thread.updated_at >= thread.created_at);
        assert_eq!(thread.archived_at, None);
        assert_eq!(thread.cwd, home.path());
        assert_eq!(thread.cli_version, "test_version");
        assert_eq!(thread.source, SessionSource::Cli);
        let git_info = thread.git_info.expect("git info should be applied");
        assert_eq!(
            git_info.commit_hash.map(|sha| sha.0),
            Some("abcdef".to_string())
        );
        assert_eq!(git_info.branch.as_deref(), Some("main"));
        assert_eq!(
            git_info.repository_url.as_deref(),
            Some("https://example.com/repo.git")
        );
        let history = thread.history.expect("history should load");
        assert_eq!(history.thread_id, thread_id);
        assert_eq!(history.items.len(), 1);
    }

    #[tokio::test]
    async fn read_thread_falls_back_to_sqlite_summary() {
        let home = TempDir::new().expect("temp dir");
        let external = TempDir::new().expect("external temp dir");
        let config = test_config(home.path());
        let store = LocalThreadStore::new(config.clone());
        let uuid = Uuid::from_u128(214);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let rollout_path = external
            .path()
            .join(format!("rollout-2025-01-03T12-00-00-{uuid}.jsonl"));
        let runtime = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let mut builder = ThreadMetadataBuilder::new(
            thread_id,
            rollout_path.clone(),
            Utc::now(),
            SessionSource::Exec,
        );
        builder.model_provider = Some("sqlite-provider".to_string());
        builder.cwd = external.path().join("workspace");
        builder.cli_version = Some("sqlite-cli".to_string());
        let mut metadata = builder.build(config.model_provider_id.as_str());
        metadata.title = "SQLite title".to_string();
        metadata.first_user_message = Some("SQLite preview".to_string());
        metadata.model = Some("sqlite-model".to_string());
        metadata.git_sha = Some("abc123".to_string());
        metadata.git_branch = Some("sqlite-branch".to_string());
        metadata.git_origin_url = Some("https://example.com/sqlite.git".to_string());
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

        assert_eq!(thread.thread_id, thread_id);
        assert_eq!(thread.rollout_path, Some(rollout_path));
        assert_eq!(thread.preview, "SQLite preview");
        assert_eq!(thread.first_user_message.as_deref(), Some("SQLite preview"));
        assert_eq!(thread.name.as_deref(), Some("SQLite title"));
        assert_eq!(thread.model_provider, "sqlite-provider");
        assert_eq!(thread.model.as_deref(), Some("sqlite-model"));
        assert_eq!(thread.cwd, external.path().join("workspace"));
        assert_eq!(thread.cli_version, "sqlite-cli");
        assert_eq!(thread.source, SessionSource::Exec);
        assert_eq!(thread.archived_at, None);
        let git_info = thread.git_info.expect("git info should be applied");
        assert_eq!(
            git_info.commit_hash.map(|sha| sha.0),
            Some("abc123".to_string())
        );
        assert_eq!(git_info.branch.as_deref(), Some("sqlite-branch"));
        assert_eq!(
            git_info.repository_url.as_deref(),
            Some("https://example.com/sqlite.git")
        );
        assert!(thread.history.is_none());
    }

    #[tokio::test]
    async fn read_thread_sqlite_fallback_can_load_history() {
        let home = TempDir::new().expect("temp dir");
        let external = TempDir::new().expect("external temp dir");
        let config = test_config(home.path());
        let store = LocalThreadStore::new(config.clone());
        let uuid = Uuid::from_u128(215);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let rollout_path = write_session_file_with_fork(
            external.path(),
            external.path().join("sessions/2025/01/03"),
            "2025-01-03T12-00-00",
            uuid,
            "History user message",
            Some("external-provider"),
            /*forked_from_id*/ None,
        )
        .expect("external session file");
        let runtime = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let mut builder =
            ThreadMetadataBuilder::new(thread_id, rollout_path, Utc::now(), SessionSource::Cli);
        builder.model_provider = Some("sqlite-provider".to_string());
        builder.cwd = external.path().to_path_buf();
        let mut metadata = builder.build(config.model_provider_id.as_str());
        metadata.first_user_message = Some("SQLite preview".to_string());
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("state db upsert should succeed");

        let thread = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: false,
                include_history: true,
            })
            .await
            .expect("read thread");

        let history = thread.history.expect("history should load");
        assert_eq!(history.thread_id, thread_id);
        assert_eq!(history.items.len(), 2);
    }

    #[tokio::test]
    async fn read_thread_sqlite_fallback_respects_include_archived() {
        let home = TempDir::new().expect("temp dir");
        let external = TempDir::new().expect("external temp dir");
        let config = test_config(home.path());
        let store = LocalThreadStore::new(config.clone());
        let uuid = Uuid::from_u128(216);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let rollout_path = external
            .path()
            .join(format!("rollout-2025-01-03T12-00-00-{uuid}.jsonl"));
        let runtime = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let mut builder =
            ThreadMetadataBuilder::new(thread_id, rollout_path, Utc::now(), SessionSource::Cli);
        builder.archived_at = Some(Utc::now());
        let mut metadata = builder.build(config.model_provider_id.as_str());
        metadata.first_user_message = Some("Archived SQLite preview".to_string());
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("state db upsert should succeed");

        let active_only_err = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: false,
                include_history: false,
            })
            .await
            .expect_err("active-only read should fail for archived metadata");
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
        assert_eq!(thread.preview, "Archived SQLite preview");
        assert!(thread.archived_at.is_some());
    }

    #[tokio::test]
    async fn read_thread_sqlite_fallback_treats_archived_rollout_path_as_archived() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let store = LocalThreadStore::new(config.clone());
        let uuid = Uuid::from_u128(220);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let rollout_path = home
            .path()
            .join(codex_rollout::ARCHIVED_SESSIONS_SUBDIR)
            .join(format!("rollout-2025-01-03T12-00-00-{uuid}.jsonl"));
        let runtime = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let mut builder =
            ThreadMetadataBuilder::new(thread_id, rollout_path, Utc::now(), SessionSource::Cli);
        builder.model_provider = Some(config.model_provider_id.clone());
        let mut metadata = builder.build(config.model_provider_id.as_str());
        metadata.archived_at = None;
        metadata.first_user_message = Some("Archived path SQLite preview".to_string());
        runtime
            .upsert_thread(&metadata)
            .await
            .expect("state db upsert should succeed");

        let active_only_err = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: false,
                include_history: false,
            })
            .await
            .expect_err("active-only read should fail for archived metadata path");
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
        assert_eq!(thread.preview, "Archived path SQLite preview");
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
