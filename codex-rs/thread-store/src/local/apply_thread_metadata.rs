use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::protocol::GitInfo;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_rollout::append_thread_name;
use codex_state::ThreadMetadata;

use super::LocalThreadStore;
use super::helpers::rollout_path_is_archived;
use super::live_writer;
use crate::ApplyThreadMetadataParams;
use crate::ThreadMetadataUpdate;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) async fn apply_thread_metadata(
    store: &LocalThreadStore,
    params: ApplyThreadMetadataParams,
) -> ThreadStoreResult<()> {
    let thread_id = params.thread_id;
    let update = params.update;
    let _metadata_permit = store.acquire_metadata_permit(thread_id).await?;
    let Some(state_db) = store.state_db().await else {
        if let Some(name) = update.name.as_ref() {
            append_thread_name_update(store, thread_id, name.as_deref().unwrap_or_default())
                .await?;
        }
        return Ok(());
    };
    flush_live_writer_if_present(store, thread_id).await?;
    let existing =
        state_db
            .get_thread(thread_id)
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to read thread metadata for {thread_id}: {err}"),
            })?;
    let metadata = merge_thread_metadata(
        store,
        thread_id,
        existing,
        update.clone(),
        update.memory_mode,
    )
    .await?;
    state_db
        .upsert_thread(&metadata)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to apply thread metadata for {thread_id}: {err}"),
        })?;
    if let Some(name) = update.name.as_ref() {
        append_thread_name_update(store, thread_id, name.as_deref().unwrap_or_default()).await?;
    }
    if let Some(git_info) = update.git_info.as_ref() {
        apply_git_info_update(state_db.as_ref(), thread_id, git_info.as_ref()).await?;
    }
    if let Some(memory_mode) = update.memory_mode {
        state_db
            .set_thread_memory_mode(thread_id, memory_mode_to_str(memory_mode))
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to apply thread memory mode for {thread_id}: {err}"),
            })?;
    }
    if let Some(dynamic_tools) = update.dynamic_tools.as_deref() {
        state_db
            .replace_dynamic_tools(thread_id, dynamic_tools)
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to replace dynamic tools for {thread_id}: {err}"),
            })?;
    }
    Ok(())
}

async fn append_thread_name_update(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    name: &str,
) -> ThreadStoreResult<()> {
    append_thread_name(store.config.codex_home.as_path(), thread_id, name)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to update thread name index: {err}"),
        })
}

async fn flush_live_writer_if_present(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<()> {
    match live_writer::flush_thread(store, thread_id).await {
        Ok(()) | Err(ThreadStoreError::ThreadNotFound { .. }) => Ok(()),
        Err(err) => Err(err),
    }
}

async fn apply_git_info_update(
    state_db: &codex_state::StateRuntime,
    thread_id: ThreadId,
    git_info: Option<&GitInfo>,
) -> ThreadStoreResult<()> {
    let (git_sha, git_branch, git_origin_url) = match git_info {
        Some(git) => (
            git.commit_hash.as_ref().map(|sha| sha.0.as_str()),
            git.branch.as_deref(),
            git.repository_url.as_deref(),
        ),
        None => (Some(""), Some(""), Some("")),
    };
    state_db
        .update_thread_git_info(
            thread_id,
            Some(git_sha),
            Some(git_branch),
            Some(git_origin_url),
        )
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to apply thread git info for {thread_id}: {err}"),
        })?;
    Ok(())
}

async fn merge_thread_metadata(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    existing: Option<ThreadMetadata>,
    update: ThreadMetadataUpdate,
    memory_mode: Option<ThreadMemoryMode>,
) -> ThreadStoreResult<ThreadMetadata> {
    let live_rollout_path = live_writer::rollout_path(store, thread_id).await.ok();
    let rollout_path = update
        .rollout_path
        .clone()
        .or_else(|| {
            existing
                .as_ref()
                .map(|metadata| metadata.rollout_path.clone())
        })
        .or(live_rollout_path)
        .ok_or_else(|| ThreadStoreError::Internal {
            message: format!("thread metadata update missing rollout path for {thread_id}"),
        })?;
    let now = Utc::now();
    let created_at = update
        .created_at
        .or_else(|| existing.as_ref().map(|metadata| metadata.created_at))
        .unwrap_or(now);
    let updated_at = update
        .updated_at
        .or_else(|| existing.as_ref().map(|metadata| metadata.updated_at))
        .unwrap_or(created_at);
    let tokens_used = token_count_from_update(&update)
        .or_else(|| existing.as_ref().map(|metadata| metadata.tokens_used))
        .unwrap_or_default();
    let archived_at = existing
        .as_ref()
        .and_then(|metadata| metadata.archived_at)
        .or_else(|| {
            rollout_path_is_archived(store.config.codex_home.as_path(), &rollout_path)
                .then_some(updated_at)
        });
    let mut metadata = ThreadMetadata {
        id: thread_id,
        rollout_path,
        created_at,
        updated_at,
        source: update
            .source
            .as_ref()
            .map(enum_to_string)
            .or_else(|| existing.as_ref().map(|metadata| metadata.source.clone()))
            .unwrap_or_else(|| enum_to_string(&SessionSource::Unknown)),
        thread_source: nested_or_existing(
            update.thread_source,
            existing
                .as_ref()
                .and_then(|metadata| metadata.thread_source),
        ),
        agent_nickname: nested_or_existing(
            update.agent_nickname,
            existing
                .as_ref()
                .and_then(|metadata| metadata.agent_nickname.clone()),
        ),
        agent_role: nested_or_existing(
            update.agent_role,
            existing
                .as_ref()
                .and_then(|metadata| metadata.agent_role.clone()),
        ),
        agent_path: nested_or_existing(
            update.agent_path,
            existing
                .as_ref()
                .and_then(|metadata| metadata.agent_path.clone()),
        ),
        model_provider: update
            .model_provider
            .or_else(|| {
                existing
                    .as_ref()
                    .map(|metadata| metadata.model_provider.clone())
            })
            .unwrap_or_else(|| store.config.default_model_provider_id.clone()),
        model: nested_or_existing(
            update.model,
            existing
                .as_ref()
                .and_then(|metadata| metadata.model.clone()),
        ),
        reasoning_effort: nested_or_existing(
            update.reasoning_effort,
            existing
                .as_ref()
                .and_then(|metadata| metadata.reasoning_effort),
        ),
        cwd: update
            .cwd
            .or_else(|| existing.as_ref().map(|metadata| metadata.cwd.clone()))
            .unwrap_or_default(),
        cli_version: update
            .cli_version
            .or_else(|| {
                existing
                    .as_ref()
                    .map(|metadata| metadata.cli_version.clone())
            })
            .unwrap_or_default(),
        title: existing
            .as_ref()
            .map(|metadata| metadata.title.clone())
            .unwrap_or_default(),
        sandbox_policy: update
            .sandbox_policy
            .as_ref()
            .map(enum_to_string)
            .or_else(|| {
                existing
                    .as_ref()
                    .map(|metadata| metadata.sandbox_policy.clone())
            })
            .unwrap_or_default(),
        approval_mode: update
            .approval_mode
            .as_ref()
            .map(enum_to_string)
            .or_else(|| {
                existing
                    .as_ref()
                    .map(|metadata| metadata.approval_mode.clone())
            })
            .unwrap_or_default(),
        tokens_used,
        first_user_message: nested_or_existing(
            update.first_user_message,
            existing
                .as_ref()
                .and_then(|metadata| metadata.first_user_message.clone()),
        ),
        archived_at,
        git_sha: None,
        git_branch: None,
        git_origin_url: None,
    };
    let name_was_explicit = update.name.is_some();
    if let Some(name) = update.name {
        metadata.title = name.unwrap_or_default();
    } else if metadata.title.is_empty()
        && let Some(preview) = update
            .preview
            .or_else(|| metadata.first_user_message.clone())
    {
        metadata.title = preview;
    }
    apply_git_info(&mut metadata, update.git_info, existing.as_ref());
    if memory_mode == Some(ThreadMemoryMode::Disabled)
        && !name_was_explicit
        && metadata.title.is_empty()
    {
        metadata.title = existing
            .as_ref()
            .map(|metadata| metadata.title.clone())
            .unwrap_or_default();
    }
    metadata.cwd = codex_rollout::state_db::normalize_cwd_for_state_db(&metadata.cwd);
    Ok(metadata)
}

fn apply_git_info(
    metadata: &mut ThreadMetadata,
    update: Option<Option<GitInfo>>,
    existing: Option<&ThreadMetadata>,
) {
    match update {
        Some(Some(git)) => {
            metadata.git_sha = git.commit_hash.map(|sha| sha.0);
            metadata.git_branch = git.branch;
            metadata.git_origin_url = git.repository_url;
        }
        Some(None) => {}
        None => {
            metadata.git_sha = existing.and_then(|metadata| metadata.git_sha.clone());
            metadata.git_branch = existing.and_then(|metadata| metadata.git_branch.clone());
            metadata.git_origin_url = existing.and_then(|metadata| metadata.git_origin_url.clone());
        }
    }
}

fn token_count_from_update(update: &ThreadMetadataUpdate) -> Option<i64> {
    match update.token_usage.as_ref() {
        Some(Some(usage)) => Some(usage.total_tokens.max(0)),
        Some(None) => Some(0),
        None => None,
    }
}

fn nested_or_existing<T>(update: Option<Option<T>>, existing: Option<T>) -> Option<T> {
    match update {
        Some(value) => value,
        None => existing,
    }
}

fn memory_mode_to_str(mode: ThreadMemoryMode) -> &'static str {
    match mode {
        ThreadMemoryMode::Enabled => "enabled",
        ThreadMemoryMode::Disabled => "disabled",
    }
}

fn enum_to_string<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use codex_protocol::dynamic_tools::DynamicToolSpec;
    use codex_protocol::protocol::GitInfo;
    use codex_protocol::protocol::TokenUsage;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;
    use crate::ThreadStore;
    use crate::local::LocalThreadStore;
    use crate::local::test_support::test_config;

    #[tokio::test]
    async fn apply_thread_metadata_updates_sqlite_metadata_and_side_tables() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config, Some(runtime.clone()));
        let thread_id = ThreadId::default();
        let rollout_path = home.path().join("rollout.jsonl");
        let created_at = Utc
            .with_ymd_and_hms(2026, 1, 2, 3, 4, 5)
            .single()
            .expect("valid timestamp");
        let updated_at = Utc
            .with_ymd_and_hms(2026, 1, 2, 4, 5, 6)
            .single()
            .expect("valid timestamp");

        store
            .apply_thread_metadata(ApplyThreadMetadataParams {
                thread_id,
                update: ThreadMetadataUpdate {
                    rollout_path: Some(rollout_path.clone()),
                    preview: Some("first preview".to_string()),
                    name: Some(Some("named thread".to_string())),
                    model_provider: Some("test-provider".to_string()),
                    model: Some(Some("test-model".to_string())),
                    created_at: Some(created_at),
                    updated_at: Some(updated_at),
                    source: Some(SessionSource::Exec),
                    cwd: Some(home.path().to_path_buf()),
                    cli_version: Some("test-version".to_string()),
                    token_usage: Some(Some(TokenUsage {
                        input_tokens: 40,
                        cached_input_tokens: 0,
                        output_tokens: 60,
                        reasoning_output_tokens: 23,
                        total_tokens: 123,
                    })),
                    first_user_message: Some(Some("first preview".to_string())),
                    git_info: Some(Some(GitInfo {
                        commit_hash: Some(codex_git_utils::GitSha::new("abcdef")),
                        branch: Some("main".to_string()),
                        repository_url: Some("https://example.com/repo.git".to_string()),
                    })),
                    memory_mode: Some(ThreadMemoryMode::Disabled),
                    dynamic_tools: Some(vec![DynamicToolSpec {
                        namespace: Some("test".to_string()),
                        name: "tool".to_string(),
                        description: "tool description".to_string(),
                        input_schema: serde_json::json!({"type": "object"}),
                        defer_loading: true,
                    }]),
                    ..Default::default()
                },
            })
            .await
            .expect("apply metadata");

        let metadata = runtime
            .get_thread(thread_id)
            .await
            .expect("read metadata")
            .expect("metadata stored");
        assert_eq!(metadata.id, thread_id);
        assert_eq!(metadata.rollout_path, rollout_path);
        assert_eq!(metadata.title, "named thread");
        assert_eq!(
            metadata.first_user_message.as_deref(),
            Some("first preview")
        );
        assert_eq!(metadata.model_provider, "test-provider");
        assert_eq!(metadata.model.as_deref(), Some("test-model"));
        assert_eq!(metadata.created_at, created_at);
        assert_eq!(metadata.updated_at, updated_at);
        assert_eq!(metadata.cli_version, "test-version");
        assert_eq!(metadata.tokens_used, 123);
        assert_eq!(metadata.git_sha.as_deref(), Some("abcdef"));
        assert_eq!(metadata.git_branch.as_deref(), Some("main"));
        assert_eq!(
            metadata.git_origin_url.as_deref(),
            Some("https://example.com/repo.git")
        );

        let memory_mode = runtime
            .get_thread_memory_mode(thread_id)
            .await
            .expect("read memory mode");
        assert_eq!(memory_mode.as_deref(), Some("disabled"));
        let dynamic_tools = runtime
            .get_dynamic_tools(thread_id)
            .await
            .expect("read dynamic tools")
            .expect("dynamic tools stored");
        assert_eq!(dynamic_tools.len(), 1);
        assert_eq!(dynamic_tools[0].name, "tool");

        store
            .apply_thread_metadata(ApplyThreadMetadataParams {
                thread_id,
                update: ThreadMetadataUpdate {
                    name: Some(None),
                    git_info: Some(None),
                    token_usage: Some(None),
                    dynamic_tools: Some(Vec::new()),
                    ..Default::default()
                },
            })
            .await
            .expect("clear metadata");

        let metadata = runtime
            .get_thread(thread_id)
            .await
            .expect("read metadata")
            .expect("metadata stored");
        assert_eq!(metadata.title, "");
        assert_eq!(metadata.tokens_used, 0);
        assert_eq!(metadata.git_sha.as_deref(), Some(""));
        assert_eq!(metadata.git_branch.as_deref(), Some(""));
        assert_eq!(metadata.git_origin_url.as_deref(), Some(""));
        let dynamic_tools = runtime
            .get_dynamic_tools(thread_id)
            .await
            .expect("read dynamic tools");
        assert_eq!(dynamic_tools, None);
    }

    #[tokio::test]
    async fn apply_thread_metadata_without_sqlite_updates_name_index() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let thread_id = ThreadId::default();

        store
            .apply_thread_metadata(ApplyThreadMetadataParams {
                thread_id,
                update: ThreadMetadataUpdate {
                    name: Some(Some("index-only name".to_string())),
                    ..Default::default()
                },
            })
            .await
            .expect("apply metadata");

        let name = codex_rollout::find_thread_name_by_id(home.path(), &thread_id)
            .await
            .expect("read thread name");
        assert_eq!(name.as_deref(), Some("index-only name"));

        store
            .apply_thread_metadata(ApplyThreadMetadataParams {
                thread_id,
                update: ThreadMetadataUpdate {
                    name: Some(None),
                    ..Default::default()
                },
            })
            .await
            .expect("clear metadata");

        let name = codex_rollout::find_thread_name_by_id(home.path(), &thread_id)
            .await
            .expect("read thread name");
        assert_eq!(name.as_deref(), Some(""));

        store
            .apply_thread_metadata(ApplyThreadMetadataParams {
                thread_id,
                update: ThreadMetadataUpdate {
                    git_info: Some(None),
                    ..Default::default()
                },
            })
            .await
            .expect("git metadata is represented by appended JSONL without sqlite");
    }

    #[tokio::test]
    async fn apply_thread_metadata_does_not_index_name_when_validation_fails() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config, Some(runtime));
        let thread_id = ThreadId::default();

        let err = store
            .apply_thread_metadata(ApplyThreadMetadataParams {
                thread_id,
                update: ThreadMetadataUpdate {
                    name: Some(Some("should not be indexed".to_string())),
                    ..Default::default()
                },
            })
            .await
            .expect_err("metadata update should fail without a rollout path");

        assert!(matches!(err, ThreadStoreError::Internal { .. }));
        let name = codex_rollout::find_thread_name_by_id(home.path(), &thread_id)
            .await
            .expect("read thread name");
        assert_eq!(name, None);
    }
}
