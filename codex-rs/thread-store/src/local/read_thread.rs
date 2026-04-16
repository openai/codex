use chrono::DateTime;
use chrono::Utc;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionMetaLine;
use codex_protocol::protocol::SessionSource;
use codex_rollout::RolloutRecorder;
use codex_rollout::find_archived_thread_path_by_id_str;
use codex_rollout::find_thread_name_by_id;
use codex_rollout::find_thread_path_by_id_str;
use codex_rollout::read_session_meta_line;
use codex_rollout::read_thread_item_from_rollout;
use codex_state::StateRuntime;
use codex_state::ThreadMetadata;

use super::LocalThreadStore;
use super::helpers::git_info_from_parts;
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
    if let Some(metadata) = read_sqlite_metadata(store, thread_id).await
        && (params.include_archived || metadata.archived_at.is_none())
    {
        let mut thread = stored_thread_from_sqlite_metadata(store, metadata).await;
        if params.include_history {
            let path = resolve_rollout_path(store, thread_id, /*include_archived*/ false)
                .await?
                .ok_or_else(|| ThreadStoreError::Internal {
                    message: format!("failed to locate rollout for thread {thread_id}"),
                })?;
            let items = load_history_items(&path).await?;
            thread.history = Some(StoredThreadHistory { thread_id, items });
        }
        return Ok(thread);
    }

    let path = resolve_rollout_path(store, thread_id, params.include_archived)
        .await?
        .ok_or_else(|| ThreadStoreError::InvalidRequest {
            message: format!("no rollout found for thread id {thread_id}"),
        })?;

    let mut thread = read_thread_from_rollout_path(store, thread_id, path).await?;
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

async fn resolve_rollout_path(
    store: &LocalThreadStore,
    thread_id: codex_protocol::ThreadId,
    include_archived: bool,
) -> ThreadStoreResult<Option<std::path::PathBuf>> {
    if include_archived {
        match find_thread_path_by_id_str(store.config.codex_home.as_path(), &thread_id.to_string())
            .await
            .map_err(|err| ThreadStoreError::InvalidRequest {
                message: format!("failed to locate thread id {thread_id}: {err}"),
            })? {
            Some(path) => Ok(Some(path)),
            None => find_archived_thread_path_by_id_str(
                store.config.codex_home.as_path(),
                &thread_id.to_string(),
            )
            .await
            .map_err(|err| ThreadStoreError::InvalidRequest {
                message: format!("failed to locate archived thread id {thread_id}: {err}"),
            }),
        }
    } else {
        find_thread_path_by_id_str(store.config.codex_home.as_path(), &thread_id.to_string())
            .await
            .map_err(|err| ThreadStoreError::InvalidRequest {
                message: format!("failed to locate thread id {thread_id}: {err}"),
            })
    }
}

async fn read_thread_from_rollout_path(
    store: &LocalThreadStore,
    thread_id: codex_protocol::ThreadId,
    path: std::path::PathBuf,
) -> ThreadStoreResult<StoredThread> {
    let Some(item) = read_thread_item_from_rollout(path.clone()).await else {
        return stored_thread_from_session_meta(store, path).await;
    };
    let archived = path.starts_with(
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
    if let Ok(Some(title)) =
        find_thread_name_by_id(store.config.codex_home.as_path(), &thread_id).await
    {
        set_thread_name_from_title(&mut thread, title);
    }
    Ok(thread)
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

async fn read_sqlite_metadata(
    store: &LocalThreadStore,
    thread_id: codex_protocol::ThreadId,
) -> Option<ThreadMetadata> {
    let runtime = StateRuntime::init(
        store.config.sqlite_home.clone(),
        store.config.model_provider_id.clone(),
    )
    .await
    .ok()?;
    runtime.get_thread(thread_id).await.ok().flatten()
}

async fn stored_thread_from_sqlite_metadata(
    store: &LocalThreadStore,
    metadata: ThreadMetadata,
) -> StoredThread {
    let name = match distinct_title(&metadata) {
        Some(title) => Some(title),
        None => find_thread_name_by_id(store.config.codex_home.as_path(), &metadata.id)
            .await
            .ok()
            .flatten(),
    };
    let forked_from_id = read_session_meta_line(metadata.rollout_path.as_path())
        .await
        .ok()
        .and_then(|meta_line| meta_line.meta.forked_from_id);
    StoredThread {
        thread_id: metadata.id,
        rollout_path: Some(metadata.rollout_path),
        forked_from_id,
        preview: metadata.first_user_message.clone().unwrap_or_default(),
        name,
        model_provider: if metadata.model_provider.is_empty() {
            store.config.model_provider_id.clone()
        } else {
            metadata.model_provider
        },
        model: metadata.model,
        reasoning_effort: metadata.reasoning_effort,
        created_at: metadata.created_at,
        updated_at: metadata.updated_at,
        archived_at: metadata.archived_at,
        cwd: metadata.cwd,
        cli_version: metadata.cli_version,
        source: parse_session_source(&metadata.source),
        agent_nickname: metadata.agent_nickname,
        agent_role: metadata.agent_role,
        agent_path: metadata.agent_path,
        git_info: git_info_from_parts(
            metadata.git_sha,
            metadata.git_branch,
            metadata.git_origin_url,
        ),
        approval_mode: parse_or_default(&metadata.approval_mode, AskForApproval::OnRequest),
        sandbox_policy: parse_or_default(
            &metadata.sandbox_policy,
            SandboxPolicy::new_read_only_policy(),
        ),
        token_usage: None,
        first_user_message: metadata.first_user_message,
        history: None,
    }
}

async fn stored_thread_from_session_meta(
    store: &LocalThreadStore,
    path: std::path::PathBuf,
) -> ThreadStoreResult<StoredThread> {
    let meta_line = read_session_meta_line(path.as_path())
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to read thread {}: {err}", path.display()),
        })?;
    let archived = path.starts_with(
        store
            .config
            .codex_home
            .join(codex_rollout::ARCHIVED_SESSIONS_SUBDIR),
    );
    Ok(stored_thread_from_meta_line(
        store, meta_line, path, archived,
    ))
}

fn stored_thread_from_meta_line(
    store: &LocalThreadStore,
    meta_line: SessionMetaLine,
    path: std::path::PathBuf,
    archived: bool,
) -> StoredThread {
    let created_at = parse_rfc3339_non_optional(&meta_line.meta.timestamp).unwrap_or_else(Utc::now);
    let updated_at = std::fs::metadata(path.as_path())
        .ok()
        .and_then(|meta| meta.modified().ok())
        .map(DateTime::<Utc>::from)
        .unwrap_or(created_at);
    StoredThread {
        thread_id: meta_line.meta.id,
        rollout_path: Some(path),
        forked_from_id: meta_line.meta.forked_from_id,
        preview: String::new(),
        name: None,
        model_provider: meta_line
            .meta
            .model_provider
            .filter(|provider| !provider.is_empty())
            .unwrap_or_else(|| store.config.model_provider_id.clone()),
        model: None,
        reasoning_effort: None,
        created_at,
        updated_at,
        archived_at: archived.then_some(updated_at),
        cwd: meta_line.meta.cwd,
        cli_version: meta_line.meta.cli_version,
        source: meta_line.meta.source,
        agent_nickname: meta_line.meta.agent_nickname,
        agent_role: meta_line.meta.agent_role,
        agent_path: meta_line.meta.agent_path,
        git_info: meta_line.git,
        approval_mode: AskForApproval::OnRequest,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        token_usage: None,
        first_user_message: None,
        history: None,
    }
}

fn distinct_title(metadata: &ThreadMetadata) -> Option<String> {
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

fn parse_session_source(source: &str) -> SessionSource {
    serde_json::from_str(source)
        .or_else(|_| serde_json::from_value(serde_json::Value::String(source.to_string())))
        .unwrap_or(SessionSource::Unknown)
}

fn parse_or_default<T>(value: &str, default: T) -> T
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(value)
        .or_else(|_| serde_json::from_value(serde_json::Value::String(value.to_string())))
        .unwrap_or(default)
}

fn parse_rfc3339_non_optional(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
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
