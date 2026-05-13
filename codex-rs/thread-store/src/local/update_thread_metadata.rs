use std::path::Path;
use std::path::PathBuf;

use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::protocol::GitInfo;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_rollout::ARCHIVED_SESSIONS_SUBDIR;
use codex_rollout::append_rollout_item_to_path;
use codex_rollout::append_thread_name;
use codex_rollout::find_archived_thread_path_by_id_str;
use codex_rollout::find_thread_path_by_id_str;
use codex_rollout::read_session_meta_line;
use codex_state::ThreadMetadataBuilder;

use super::LocalThreadStore;
use super::helpers::git_info_from_parts;
use super::live_writer;
use crate::GitInfoPatch;
use crate::ReadThreadParams;
use crate::StoredThread;
use crate::ThreadMetadataPatch;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::UpdateThreadMetadataParams;
use crate::local::read_thread;

struct ResolvedRolloutPath {
    path: PathBuf,
    archived: bool,
}

pub(super) async fn update_thread_metadata(
    store: &LocalThreadStore,
    params: UpdateThreadMetadataParams,
) -> ThreadStoreResult<StoredThread> {
    let thread_id = params.thread_id;
    let patch = params.patch;
    if patch.is_empty() {
        return read_thread::read_thread(
            store,
            ReadThreadParams {
                thread_id,
                include_archived: params.include_archived,
                include_history: false,
            },
        )
        .await;
    }

    let needs_rollout_compat = needs_rollout_compatibility_update(&patch);
    let updated =
        apply_metadata_update(store, thread_id, patch.clone(), params.include_archived).await?;
    if !needs_rollout_compat {
        return Ok(updated);
    }

    if live_writer::rollout_path(store, thread_id).await.is_ok() {
        live_writer::persist_thread(store, thread_id).await?;
    }
    let resolved_rollout_path =
        resolve_rollout_path(store, thread_id, params.include_archived).await?;
    let name = patch.name;
    let git_info = patch.git_info;
    if let Some(memory_mode) = patch.memory_mode {
        apply_thread_memory_mode(resolved_rollout_path.path.as_path(), thread_id, memory_mode)
            .await?;
    }

    let state_db_ctx = store.state_db().await;
    codex_rollout::state_db::reconcile_rollout(
        state_db_ctx.as_deref(),
        resolved_rollout_path.path.as_path(),
        store.config.default_model_provider_id.as_str(),
        /*builder*/ None,
        &[],
        /*archived_only*/ resolved_rollout_path.archived.then_some(true),
        /*new_thread_memory_mode*/ None,
    )
    .await;

    if let Some(name) = name {
        apply_thread_name(store, thread_id, name.unwrap_or_default()).await?;
    }

    let resolved_git_info = match git_info {
        Some(git_info) => {
            let Some(state_db) = store.state_db().await else {
                return Err(ThreadStoreError::Internal {
                    message: format!("sqlite state db unavailable for thread {thread_id}"),
                });
            };
            let metadata =
                state_db
                    .get_thread(thread_id)
                    .await
                    .map_err(|err| ThreadStoreError::Internal {
                        message: format!(
                            "failed to read git metadata for thread {thread_id}: {err}"
                        ),
                    })?;
            let Some(metadata) = metadata else {
                return Err(ThreadStoreError::Internal {
                    message: format!("thread metadata unavailable before git update: {thread_id}"),
                });
            };
            let memory_mode = state_db
                .get_thread_memory_mode(thread_id)
                .await
                .map_err(|err| ThreadStoreError::Internal {
                    message: format!("failed to read memory mode for thread {thread_id}: {err}"),
                })?;
            let existing_git_info = git_info_from_parts(
                metadata.git_sha,
                metadata.git_branch,
                metadata.git_origin_url,
            );
            Some((
                resolve_git_info_patch(existing_git_info, git_info),
                memory_mode,
            ))
        }
        None => None,
    };
    if let Some(((sha, branch, origin_url), memory_mode)) = resolved_git_info.as_ref() {
        apply_thread_git_info_to_rollout(
            resolved_rollout_path.path.as_path(),
            thread_id,
            sha,
            branch,
            origin_url,
            memory_mode.as_deref(),
        )
        .await?;
        apply_thread_git_info(store, thread_id, sha, branch, origin_url).await?;
    }

    let mut thread = match read_thread::read_thread(
        store,
        ReadThreadParams {
            thread_id,
            include_archived: params.include_archived,
            include_history: false,
        },
    )
    .await
    {
        Ok(thread) => thread,
        Err(_) => {
            read_thread::read_thread_by_rollout_path(
                store,
                resolved_rollout_path.path,
                params.include_archived,
                /*include_history*/ false,
            )
            .await?
        }
    };
    if let Some(((sha, branch, origin_url), _memory_mode)) = resolved_git_info {
        thread.git_info = git_info_from_parts(sha, branch, origin_url);
    }
    Ok(thread)
}

async fn apply_metadata_update(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    patch: ThreadMetadataPatch,
    include_archived: bool,
) -> ThreadStoreResult<StoredThread> {
    let live_rollout_path = live_writer::rollout_path(store, thread_id).await.ok();
    let mut rollout_path = patch.rollout_path.clone().or(live_rollout_path);
    let mut rollout_path_archived = rollout_path
        .as_deref()
        .is_some_and(|path| rollout_path_is_archived(store, path));
    let state_db = store.state_db().await;
    let sqlite_write_result: ThreadStoreResult<()> = if let Some(state_db) = state_db.as_ref() {
        let patch = patch.clone();
        async {
            let existing =
                state_db
                    .get_thread(thread_id)
                    .await
                    .map_err(|err| ThreadStoreError::Internal {
                        message: format!("failed to read thread metadata for {thread_id}: {err}"),
                    })?;
            if existing.is_none() && rollout_path.is_none() {
                let resolved = resolve_rollout_path(store, thread_id, include_archived).await?;
                rollout_path_archived = resolved.archived;
                rollout_path = Some(resolved.path);
            }
            let mut metadata = existing.clone().unwrap_or_else(|| {
                let created_at = patch
                    .created_at
                    .or(patch.updated_at)
                    .unwrap_or_else(Utc::now);
                let mut builder = ThreadMetadataBuilder::new(
                    thread_id,
                    rollout_path.clone().unwrap_or_default(),
                    created_at,
                    patch.source.clone().unwrap_or(SessionSource::Unknown),
                );
                builder.model_provider = patch.model_provider.clone();
                builder.thread_source = patch.thread_source.flatten();
                builder.agent_nickname = patch.agent_nickname.clone().flatten();
                builder.agent_role = patch.agent_role.clone().flatten();
                builder.agent_path = patch.agent_path.clone().flatten();
                builder.cwd = patch.cwd.clone().map(normalize_cwd).unwrap_or_default();
                builder.cli_version = patch.cli_version.clone();
                let mut metadata = builder.build(store.config.default_model_provider_id.as_str());
                if rollout_path_archived {
                    metadata.archived_at = Some(metadata.updated_at);
                }
                metadata
            });
            if let Some(rollout_path) = rollout_path {
                metadata.rollout_path = rollout_path;
            }
            if let Some(preview) = patch.preview {
                metadata.preview = Some(preview);
            }
            if let Some(name) = patch.name {
                metadata.title = name.unwrap_or_default();
            }
            if let Some(title) = patch.title {
                metadata.title = title;
            }
            if let Some(model_provider) = patch.model_provider {
                metadata.model_provider = model_provider;
            }
            if let Some(model) = patch.model {
                metadata.model = Some(model);
            }
            if let Some(reasoning_effort) = patch.reasoning_effort {
                metadata.reasoning_effort = Some(reasoning_effort);
            }
            if let Some(created_at) = patch.created_at {
                metadata.created_at = created_at;
            }
            if let Some(updated_at) = patch.updated_at {
                metadata.updated_at = updated_at;
            }
            if let Some(source) = patch.source {
                metadata.source = enum_to_string(&source);
            }
            if let Some(thread_source) = patch.thread_source {
                metadata.thread_source = thread_source;
            }
            if let Some(agent_nickname) = patch.agent_nickname {
                metadata.agent_nickname = agent_nickname;
            }
            if let Some(agent_role) = patch.agent_role {
                metadata.agent_role = agent_role;
            }
            if let Some(agent_path) = patch.agent_path {
                metadata.agent_path = agent_path;
            }
            if let Some(cwd) = patch.cwd {
                metadata.cwd = normalize_cwd(cwd);
            }
            if let Some(cli_version) = patch.cli_version {
                metadata.cli_version = cli_version;
            }
            if let Some(approval_mode) = patch.approval_mode {
                metadata.approval_mode = enum_to_string(&approval_mode);
            }
            if let Some(sandbox_policy) = patch.sandbox_policy {
                metadata.sandbox_policy = enum_to_string(&sandbox_policy);
            }
            if let Some(token_usage) = patch.token_usage {
                metadata.tokens_used = token_usage.total_tokens.max(0);
            }
            if let Some(first_user_message) = patch.first_user_message {
                metadata.first_user_message = Some(first_user_message);
            }
            if let Some(git_info) = patch.git_info {
                let existing_git_info = git_info_from_parts(
                    metadata.git_sha.clone(),
                    metadata.git_branch.clone(),
                    metadata.git_origin_url.clone(),
                );
                let (sha, branch, origin_url) = resolve_git_info_patch(existing_git_info, git_info);
                metadata.git_sha = sha;
                metadata.git_branch = branch;
                metadata.git_origin_url = origin_url;
            }
            state_db
                .upsert_thread(&metadata)
                .await
                .map_err(|err| ThreadStoreError::Internal {
                    message: format!("failed to update thread metadata for {thread_id}: {err}"),
                })?;
            if let Some(memory_mode) = patch.memory_mode {
                state_db
                    .set_thread_memory_mode(thread_id, memory_mode_as_str(memory_mode))
                    .await
                    .map_err(|err| ThreadStoreError::Internal {
                        message: format!("failed to update memory mode for {thread_id}: {err}"),
                    })?;
            }
            if let Some(dynamic_tools) = patch.dynamic_tools {
                state_db
                    .persist_dynamic_tools(thread_id, Some(dynamic_tools.as_slice()))
                    .await
                    .map_err(|err| ThreadStoreError::Internal {
                        message: format!("failed to update dynamic tools for {thread_id}: {err}"),
                    })?;
            }
            Ok(())
        }
        .await
    } else {
        Ok(())
    };
    match (state_db.is_some(), sqlite_write_result) {
        (true, Ok(())) => {}
        (true, Err(err)) => return Err(err),
        (false, Ok(())) => {}
        (false, Err(err)) => return Err(err),
    }

    read_thread::read_thread(
        store,
        ReadThreadParams {
            thread_id,
            include_archived,
            include_history: false,
        },
    )
    .await
}

fn needs_rollout_compatibility_update(patch: &ThreadMetadataPatch) -> bool {
    if patch.name.is_some() {
        return true;
    }
    if patch.memory_mode.is_none() && patch.git_info.is_none() {
        return false;
    }
    !has_observed_metadata_facts(patch)
}

fn has_observed_metadata_facts(patch: &ThreadMetadataPatch) -> bool {
    patch.rollout_path.is_some()
        || patch.preview.is_some()
        || patch.title.is_some()
        || patch.model_provider.is_some()
        || patch.model.is_some()
        || patch.reasoning_effort.is_some()
        || patch.created_at.is_some()
        || patch.source.is_some()
        || patch.thread_source.is_some()
        || patch.agent_nickname.is_some()
        || patch.agent_role.is_some()
        || patch.agent_path.is_some()
        || patch.cwd.is_some()
        || patch.cli_version.is_some()
        || patch.approval_mode.is_some()
        || patch.sandbox_policy.is_some()
        || patch.token_usage.is_some()
        || patch.first_user_message.is_some()
        || patch.dynamic_tools.is_some()
}

fn enum_to_string<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(value)) => value,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

fn normalize_cwd(cwd: PathBuf) -> PathBuf {
    codex_utils_path::normalize_for_path_comparison(cwd.as_path()).unwrap_or(cwd)
}

async fn apply_thread_git_info(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    sha: &Option<String>,
    branch: &Option<String>,
    origin_url: &Option<String>,
) -> ThreadStoreResult<()> {
    let Some(state_db) = store.state_db().await else {
        return Err(ThreadStoreError::Internal {
            message: format!("sqlite state db unavailable for thread {thread_id}"),
        });
    };
    let updated = state_db
        .update_thread_git_info(
            thread_id,
            Some(sha.as_deref()),
            Some(branch.as_deref()),
            Some(origin_url.as_deref()),
        )
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to update git metadata for thread {thread_id}: {err}"),
        })?;
    if updated {
        Ok(())
    } else {
        Err(ThreadStoreError::Internal {
            message: format!("thread metadata disappeared before update completed: {thread_id}"),
        })
    }
}

fn resolve_git_info_patch(
    existing: Option<GitInfo>,
    git_info: GitInfoPatch,
) -> (Option<String>, Option<String>, Option<String>) {
    let (existing_sha, existing_branch, existing_origin_url) = match existing {
        Some(info) => (
            info.commit_hash.map(|sha| sha.0),
            info.branch,
            info.repository_url,
        ),
        None => (None, None, None),
    };
    let sha = git_info.sha.unwrap_or(existing_sha);
    let branch = git_info.branch.unwrap_or(existing_branch);
    let origin_url = git_info.origin_url.unwrap_or(existing_origin_url);
    (sha, branch, origin_url)
}

async fn apply_thread_git_info_to_rollout(
    rollout_path: &Path,
    thread_id: ThreadId,
    sha: &Option<String>,
    branch: &Option<String>,
    origin_url: &Option<String>,
    memory_mode: Option<&str>,
) -> ThreadStoreResult<()> {
    let mut session_meta =
        read_session_meta_line(rollout_path)
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to set thread git metadata: {err}"),
            })?;
    if session_meta.meta.id != thread_id {
        return Err(ThreadStoreError::Internal {
            message: format!(
                "failed to set thread git metadata: rollout session metadata id mismatch: expected {thread_id}, found {}",
                session_meta.meta.id
            ),
        });
    }

    session_meta.git = Some(GitInfo {
        commit_hash: sha.as_deref().map(codex_git_utils::GitSha::new),
        branch: branch.clone(),
        repository_url: origin_url.clone(),
    });
    session_meta.meta.memory_mode = memory_mode.map(str::to_string);
    append_rollout_item_to_path(rollout_path, &RolloutItem::SessionMeta(session_meta))
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to set thread git metadata: {err}"),
        })
}

async fn apply_thread_name(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    name: String,
) -> ThreadStoreResult<()> {
    if let Some(state_db) = store.state_db().await {
        let updated = state_db
            .update_thread_title(thread_id, &name)
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to set thread name: {err}"),
            })?;
        if !updated {
            return Err(ThreadStoreError::Internal {
                message: format!("thread metadata unavailable before name update: {thread_id}"),
            });
        }
    }

    append_thread_name(store.config.codex_home.as_path(), thread_id, &name)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to index thread name: {err}"),
        })
}

async fn apply_thread_memory_mode(
    rollout_path: &Path,
    thread_id: ThreadId,
    memory_mode: ThreadMemoryMode,
) -> ThreadStoreResult<()> {
    let mut session_meta =
        read_session_meta_line(rollout_path)
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to set thread memory mode: {err}"),
            })?;
    if session_meta.meta.id != thread_id {
        return Err(ThreadStoreError::Internal {
            message: format!(
                "failed to set thread memory mode: rollout session metadata id mismatch: expected {thread_id}, found {}",
                session_meta.meta.id
            ),
        });
    }

    // Memory-mode updates should not modify git metadata. The rollout replay
    // code will preserve the latest prior git marker when this field is absent.
    session_meta.git = None;
    session_meta.meta.memory_mode = Some(memory_mode_as_str(memory_mode).to_string());
    append_rollout_item_to_path(rollout_path, &RolloutItem::SessionMeta(session_meta))
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to set thread memory mode: {err}"),
        })
}

fn memory_mode_as_str(mode: ThreadMemoryMode) -> &'static str {
    match mode {
        ThreadMemoryMode::Enabled => "enabled",
        ThreadMemoryMode::Disabled => "disabled",
    }
}

async fn resolve_rollout_path(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    include_archived: bool,
) -> ThreadStoreResult<ResolvedRolloutPath> {
    if let Ok(path) = live_writer::rollout_path(store, thread_id).await {
        let archived = rollout_path_is_archived(store, path.as_path());
        return Ok(ResolvedRolloutPath { path, archived });
    }

    let state_db_ctx = store.state_db().await;
    let active_path = find_thread_path_by_id_str(
        store.config.codex_home.as_path(),
        &thread_id.to_string(),
        state_db_ctx.as_deref(),
    )
    .await
    .map_err(|err| ThreadStoreError::InvalidRequest {
        message: format!("failed to locate thread id {thread_id}: {err}"),
    })?;
    if let Some(path) = active_path {
        return Ok(ResolvedRolloutPath {
            path,
            archived: false,
        });
    }
    if !include_archived {
        return Err(ThreadStoreError::InvalidRequest {
            message: format!("thread not found: {thread_id}"),
        });
    }
    find_archived_thread_path_by_id_str(
        store.config.codex_home.as_path(),
        &thread_id.to_string(),
        state_db_ctx.as_deref(),
    )
    .await
    .map_err(|err| ThreadStoreError::InvalidRequest {
        message: format!("failed to locate archived thread id {thread_id}: {err}"),
    })?
    .map(|path| ResolvedRolloutPath {
        path,
        archived: true,
    })
    .ok_or_else(|| ThreadStoreError::InvalidRequest {
        message: format!("thread not found: {thread_id}"),
    })
}

fn rollout_path_is_archived(store: &LocalThreadStore, path: &Path) -> bool {
    path.starts_with(store.config.codex_home.join(ARCHIVED_SESSIONS_SUBDIR))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::Value;
    use serde_json::json;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::*;
    use crate::GitInfoPatch;
    use crate::ListThreadsParams;
    use crate::ResumeThreadParams;
    use crate::SortDirection;
    use crate::ThreadEventPersistenceMode;
    use crate::ThreadMetadataPatch;
    use crate::ThreadPersistenceMetadata;
    use crate::ThreadSortKey;
    use crate::ThreadStore;
    use crate::local::LocalThreadStore;
    use crate::local::test_support::test_config;
    use crate::local::test_support::write_archived_session_file;
    use crate::local::test_support::write_session_file;

    #[tokio::test]
    async fn update_thread_metadata_sets_name_on_active_rollout_and_indexes_name() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let uuid = Uuid::from_u128(301);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        write_session_file(home.path(), "2025-01-03T14-00-00", uuid).expect("session file");

        let thread = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    name: Some(Some("A sharper name".to_string())),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("set thread name");

        assert_eq!(thread.name.as_deref(), Some("A sharper name"));
        let latest_name = codex_rollout::find_thread_name_by_id(home.path(), &thread_id)
            .await
            .expect("find thread name");
        assert_eq!(latest_name.as_deref(), Some("A sharper name"));
    }

    #[tokio::test]
    async fn update_thread_metadata_sets_memory_mode_on_active_rollout() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let uuid = Uuid::from_u128(302);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let path =
            write_session_file(home.path(), "2025-01-03T14-30-00", uuid).expect("session file");
        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config.clone(), Some(runtime.clone()));

        let thread = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    memory_mode: Some(ThreadMemoryMode::Disabled),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("set thread memory mode");

        assert_eq!(thread.thread_id, thread_id);
        let appended = last_rollout_item(path.as_path());
        assert_eq!(appended["type"], "session_meta");
        assert_eq!(appended["payload"]["id"], thread_id.to_string());
        assert_eq!(appended["payload"]["memory_mode"], "disabled");
        let memory_mode = runtime
            .get_thread_memory_mode(thread_id)
            .await
            .expect("thread memory mode should be readable");
        assert_eq!(memory_mode.as_deref(), Some("disabled"));
    }

    #[tokio::test]
    async fn update_thread_metadata_preserves_memory_mode_when_updating_git_info() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let uuid = Uuid::from_u128(312);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let path =
            write_session_file(home.path(), "2025-01-03T18-30-00", uuid).expect("session file");
        let runtime = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config.clone(), Some(runtime.clone()));

        store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    memory_mode: Some(ThreadMemoryMode::Disabled),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("set memory mode");

        let thread = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    git_info: Some(GitInfoPatch {
                        branch: Some(Some("feature".to_string())),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("set git metadata");

        assert_eq!(
            thread.git_info.expect("git info").branch.as_deref(),
            Some("feature")
        );
        let appended = last_rollout_item(path.as_path());
        assert_eq!(appended["type"], "session_meta");
        assert_eq!(appended["payload"]["memory_mode"], "disabled");
        assert_eq!(appended["payload"]["git"]["branch"], "feature");

        codex_rollout::state_db::reconcile_rollout(
            Some(runtime.as_ref()),
            path.as_path(),
            config.default_model_provider_id.as_str(),
            /*builder*/ None,
            &[],
            /*archived_only*/ None,
            /*new_thread_memory_mode*/ None,
        )
        .await;
        let memory_mode = runtime
            .get_thread_memory_mode(thread_id)
            .await
            .expect("thread memory mode should be readable");
        assert_eq!(memory_mode.as_deref(), Some("disabled"));
    }

    #[tokio::test]
    async fn update_thread_metadata_uses_live_rollout_path_for_external_resume() {
        let home = TempDir::new().expect("temp dir");
        let external_home = TempDir::new().expect("external temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let uuid = Uuid::from_u128(307);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let path = write_session_file(external_home.path(), "2025-01-03T14-45-00", uuid)
            .expect("external session file");

        store
            .resume_thread(ResumeThreadParams {
                thread_id,
                rollout_path: Some(path.clone()),
                history: None,
                include_archived: true,
                metadata: test_thread_metadata(),
                event_persistence_mode: ThreadEventPersistenceMode::Limited,
            })
            .await
            .expect("resume external live thread");

        let thread = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    memory_mode: Some(ThreadMemoryMode::Disabled),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("set memory mode on external live thread");

        assert_eq!(thread.thread_id, thread_id);
        assert!(thread.rollout_path.is_some());
        let appended = last_rollout_item(path.as_path());
        assert_eq!(appended["type"], "session_meta");
        assert_eq!(appended["payload"]["memory_mode"], "disabled");
    }

    #[tokio::test]
    async fn update_thread_metadata_sets_git_info() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let runtime = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config, Some(runtime));
        let uuid = Uuid::from_u128(309);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        write_session_file(home.path(), "2025-01-03T17-00-00", uuid).expect("session file");

        let thread = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    git_info: Some(GitInfoPatch {
                        sha: Some(Some("abc123".to_string())),
                        branch: Some(Some("main".to_string())),
                        origin_url: Some(Some("https://github.com/openai/codex".to_string())),
                    }),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("set git metadata");

        let git_info = thread.git_info.expect("git info should be present");
        assert_eq!(
            git_info.commit_hash.as_ref().map(|sha| sha.0.as_str()),
            Some("abc123")
        );
        assert_eq!(git_info.branch.as_deref(), Some("main"));
        assert_eq!(
            git_info.repository_url.as_deref(),
            Some("https://github.com/openai/codex")
        );
    }

    #[tokio::test]
    async fn update_thread_metadata_partially_updates_git_info() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let runtime = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config, Some(runtime));
        let uuid = Uuid::from_u128(310);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        write_session_file(home.path(), "2025-01-03T17-30-00", uuid).expect("session file");

        store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    git_info: Some(GitInfoPatch {
                        sha: Some(Some("abc123".to_string())),
                        branch: Some(Some("main".to_string())),
                        origin_url: Some(Some("https://github.com/openai/codex".to_string())),
                    }),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("seed git metadata");

        let thread = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    git_info: Some(GitInfoPatch {
                        branch: Some(Some("feature".to_string())),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("partially update git metadata");

        let git_info = thread.git_info.expect("git info should be present");
        assert_eq!(
            git_info.commit_hash.as_ref().map(|sha| sha.0.as_str()),
            Some("abc123")
        );
        assert_eq!(git_info.branch.as_deref(), Some("feature"));
        assert_eq!(
            git_info.repository_url.as_deref(),
            Some("https://github.com/openai/codex")
        );
    }

    #[tokio::test]
    async fn update_thread_metadata_clears_git_info_fields() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let runtime = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config.clone(), Some(runtime.clone()));
        let uuid = Uuid::from_u128(311);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let path =
            write_session_file(home.path(), "2025-01-03T18-00-00", uuid).expect("session file");

        store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    git_info: Some(GitInfoPatch {
                        sha: Some(Some("abc123".to_string())),
                        branch: Some(Some("main".to_string())),
                        origin_url: Some(Some("https://github.com/openai/codex".to_string())),
                    }),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("seed git metadata");

        let thread = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    git_info: Some(GitInfoPatch {
                        sha: Some(None),
                        branch: Some(None),
                        origin_url: Some(None),
                    }),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("clear git metadata");

        assert!(thread.git_info.is_none());
        let appended = last_rollout_item(path.as_path());
        assert_eq!(appended["type"], "session_meta");
        assert_eq!(appended["payload"]["git"], json!({}));

        codex_rollout::state_db::reconcile_rollout(
            Some(runtime.as_ref()),
            path.as_path(),
            config.default_model_provider_id.as_str(),
            /*builder*/ None,
            &[],
            /*archived_only*/ None,
            /*new_thread_memory_mode*/ None,
        )
        .await;
        let thread = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: false,
                include_history: false,
            })
            .await
            .expect("read thread after reconcile");
        assert!(thread.git_info.is_none());

        store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    memory_mode: Some(ThreadMemoryMode::Disabled),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("set memory mode after git clear");
        let appended = last_rollout_item(path.as_path());
        assert_eq!(appended["type"], "session_meta");
        assert_eq!(appended["payload"].get("git"), None);
        codex_rollout::state_db::reconcile_rollout(
            Some(runtime.as_ref()),
            path.as_path(),
            config.default_model_provider_id.as_str(),
            /*builder*/ None,
            &[],
            /*archived_only*/ None,
            /*new_thread_memory_mode*/ None,
        )
        .await;
        let thread = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: false,
                include_history: false,
            })
            .await
            .expect("read thread after memory mode update with no git");
        assert!(thread.git_info.is_none());

        assert_eq!(
            runtime
                .delete_thread(thread_id)
                .await
                .expect("delete sqlite thread row"),
            1
        );
        let thread = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    git_info: Some(GitInfoPatch {
                        branch: Some(Some("feature".to_string())),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("partially update after clear with missing sqlite row");
        let git_info = thread.git_info.expect("branch should be present");
        assert_eq!(git_info.commit_hash, None);
        assert_eq!(git_info.branch.as_deref(), Some("feature"));
        assert_eq!(git_info.repository_url, None);

        store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    memory_mode: Some(ThreadMemoryMode::Disabled),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("set memory mode after git clear and partial update");
        let appended = last_rollout_item(path.as_path());
        assert_eq!(appended["type"], "session_meta");
        assert_eq!(appended["payload"].get("git"), None);
        codex_rollout::state_db::reconcile_rollout(
            Some(runtime.as_ref()),
            path.as_path(),
            config.default_model_provider_id.as_str(),
            /*builder*/ None,
            &[],
            /*archived_only*/ None,
            /*new_thread_memory_mode*/ None,
        )
        .await;
        let thread = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_archived: false,
                include_history: false,
            })
            .await
            .expect("read thread after memory mode update");
        let git_info = thread.git_info.expect("branch should remain present");
        assert_eq!(git_info.commit_hash, None);
        assert_eq!(git_info.branch.as_deref(), Some("feature"));
        assert_eq!(git_info.repository_url, None);
    }

    #[tokio::test]
    async fn update_thread_metadata_rejects_mismatched_session_meta_id() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let filename_uuid = Uuid::from_u128(303);
        let metadata_uuid = Uuid::from_u128(304);
        let thread_id = ThreadId::from_string(&filename_uuid.to_string()).expect("valid thread id");
        let path = write_session_file(home.path(), "2025-01-03T15-00-00", filename_uuid)
            .expect("session file");
        let content = std::fs::read_to_string(&path).expect("read rollout");
        std::fs::write(
            &path,
            content.replace(&filename_uuid.to_string(), &metadata_uuid.to_string()),
        )
        .expect("rewrite rollout");

        let err = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    memory_mode: Some(ThreadMemoryMode::Enabled),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect_err("mismatch should fail");

        assert!(matches!(err, ThreadStoreError::Internal { .. }));
        assert!(err.to_string().contains("metadata id mismatch"));
    }

    #[tokio::test]
    async fn update_thread_metadata_applies_combined_explicit_patch() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config, Some(runtime.clone()));
        let uuid = Uuid::from_u128(305);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let path =
            write_session_file(home.path(), "2025-01-03T15-30-00", uuid).expect("session file");

        let thread = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    name: Some(Some("Combined metadata".to_string())),
                    memory_mode: Some(ThreadMemoryMode::Disabled),
                    git_info: Some(GitInfoPatch {
                        branch: Some(Some("combined".to_string())),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("combined patch should apply");

        assert_eq!(thread.name.as_deref(), Some("Combined metadata"));
        assert_eq!(
            thread.git_info.expect("git info").branch.as_deref(),
            Some("combined")
        );
        let appended = last_rollout_item(path.as_path());
        assert_eq!(appended["type"], "session_meta");
        assert_eq!(appended["payload"]["memory_mode"], "disabled");
        assert_eq!(appended["payload"]["git"]["branch"], "combined");
        let latest_name = codex_rollout::find_thread_name_by_id(home.path(), &thread_id)
            .await
            .expect("find thread name");
        assert_eq!(latest_name.as_deref(), Some("Combined metadata"));
        let memory_mode = runtime
            .get_thread_memory_mode(thread_id)
            .await
            .expect("thread memory mode should be readable");
        assert_eq!(memory_mode.as_deref(), Some("disabled"));
    }

    #[tokio::test]
    async fn metadata_patch_applies_title_over_existing_name() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config, Some(runtime));
        let uuid = Uuid::from_u128(306);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        write_session_file(home.path(), "2025-01-03T15-45-00", uuid).expect("session file");

        store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    name: Some(Some("User chosen name".to_string())),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("set explicit name");

        let thread = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    title: Some("Derived first message".to_string()),
                    preview: Some("Derived first message".to_string()),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("apply observed metadata");

        assert_eq!(thread.name.as_deref(), Some("Derived first message"));
    }

    #[tokio::test]
    async fn metadata_patch_applies_latest_preview_and_first_user_message() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config, Some(runtime.clone()));
        let uuid = Uuid::from_u128(313);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        write_session_file(home.path(), "2025-01-03T19-00-00", uuid).expect("session file");

        store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    preview: Some("Original preview".to_string()),
                    first_user_message: Some("Original first message".to_string()),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("set observed metadata");

        let thread = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    preview: Some("Later preview".to_string()),
                    first_user_message: Some("Later first message".to_string()),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("apply later observed metadata");

        assert_eq!(thread.preview, "Hello from user");
        assert_eq!(
            thread.first_user_message.as_deref(),
            Some("Hello from user")
        );
        let metadata = runtime
            .get_thread(thread_id)
            .await
            .expect("sqlite metadata read")
            .expect("sqlite metadata");
        assert_eq!(metadata.preview.as_deref(), Some("Later preview"));
        assert_eq!(
            metadata.first_user_message.as_deref(),
            Some("Later first message")
        );
    }

    #[tokio::test]
    async fn observed_metadata_rejects_unknown_thread_without_rollout() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config, Some(runtime.clone()));
        let uuid = Uuid::from_u128(314);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");

        let err = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    preview: Some("phantom".to_string()),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect_err("metadata-only update should not create a missing thread");

        assert!(matches!(
            err,
            ThreadStoreError::InvalidRequest { message }
                if message == format!("thread not found: {thread_id}")
        ));
        let metadata = runtime
            .get_thread(thread_id)
            .await
            .expect("sqlite metadata read");
        assert!(metadata.is_none());
    }

    #[tokio::test]
    async fn update_thread_metadata_recreates_missing_archived_sqlite_row_as_archived() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let uuid = Uuid::from_u128(315);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        write_archived_session_file(home.path(), "2025-01-03T19-30-00", uuid)
            .expect("archived session file");
        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config, Some(runtime.clone()));

        let thread = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    preview: Some("Archived missing sqlite row".to_string()),
                    ..Default::default()
                },
                include_archived: true,
            })
            .await
            .expect("update archived thread without sqlite row");

        assert!(thread.archived_at.is_some());
        assert!(
            runtime
                .get_thread(thread_id)
                .await
                .expect("get metadata")
                .expect("metadata")
                .archived_at
                .is_some()
        );
    }

    #[tokio::test]
    async fn observed_metadata_normalizes_cwd_for_list_filters() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config, Some(runtime.clone()));
        let uuid = Uuid::from_u128(316);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        write_session_file(home.path(), "2025-01-03T20-00-00", uuid).expect("session file");
        let workspace = home.path().join("workspace");
        let child = workspace.join("child");
        std::fs::create_dir_all(child.as_path()).expect("create workspace");
        let unnormalized_cwd = child.join("..");
        let normalized_cwd = codex_utils_path::normalize_for_path_comparison(workspace.as_path())
            .expect("normalize cwd");

        store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    cwd: Some(unnormalized_cwd),
                    preview: Some("cwd preview".to_string()),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect("update observed cwd");

        let metadata = runtime
            .get_thread(thread_id)
            .await
            .expect("get metadata")
            .expect("metadata");
        assert_eq!(metadata.cwd, normalized_cwd);
        let page = store
            .list_threads(ListThreadsParams {
                page_size: 10,
                cursor: None,
                sort_key: ThreadSortKey::UpdatedAt,
                sort_direction: SortDirection::Desc,
                allowed_sources: Vec::new(),
                model_providers: Some(Vec::new()),
                cwd_filters: Some(vec![workspace]),
                archived: false,
                search_term: None,
                use_state_db_only: true,
            })
            .await
            .expect("list threads by cwd");
        assert_eq!(
            page.items
                .iter()
                .map(|thread| thread.thread_id)
                .collect::<Vec<_>>(),
            vec![thread_id]
        );
    }

    #[tokio::test]
    async fn update_thread_metadata_keeps_archived_thread_archived_in_sqlite() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let uuid = Uuid::from_u128(307);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let archived_path = write_archived_session_file(home.path(), "2025-01-03T16-00-00", uuid)
            .expect("archived session file");
        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config.clone(), Some(runtime.clone()));
        runtime
            .mark_backfill_complete(/*last_watermark*/ None)
            .await
            .expect("backfill should be complete");
        codex_rollout::state_db::reconcile_rollout(
            Some(runtime.as_ref()),
            archived_path.as_path(),
            config.default_model_provider_id.as_str(),
            /*builder*/ None,
            &[],
            /*archived_only*/ Some(true),
            /*new_thread_memory_mode*/ None,
        )
        .await;
        assert!(
            runtime
                .get_thread(thread_id)
                .await
                .expect("get metadata")
                .expect("metadata")
                .archived_at
                .is_some()
        );

        let thread = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    name: Some(Some("Archived title".to_string())),
                    ..Default::default()
                },
                include_archived: true,
            })
            .await
            .expect("set archived thread name");

        assert!(thread.archived_at.is_some());
        assert!(
            runtime
                .get_thread(thread_id)
                .await
                .expect("get metadata")
                .expect("metadata")
                .archived_at
                .is_some()
        );
    }

    #[tokio::test]
    async fn update_thread_metadata_keeps_live_archived_thread_archived_in_sqlite() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let uuid = Uuid::from_u128(308);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let archived_path = write_archived_session_file(home.path(), "2025-01-03T16-30-00", uuid)
            .expect("archived session file");
        let runtime = codex_state::StateRuntime::init(
            home.path().to_path_buf(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config.clone(), Some(runtime.clone()));
        runtime
            .mark_backfill_complete(/*last_watermark*/ None)
            .await
            .expect("backfill should be complete");
        codex_rollout::state_db::reconcile_rollout(
            Some(runtime.as_ref()),
            archived_path.as_path(),
            config.default_model_provider_id.as_str(),
            /*builder*/ None,
            &[],
            /*archived_only*/ Some(true),
            /*new_thread_memory_mode*/ None,
        )
        .await;
        store
            .resume_thread(ResumeThreadParams {
                thread_id,
                rollout_path: Some(archived_path.clone()),
                history: None,
                include_archived: true,
                metadata: test_thread_metadata(),
                event_persistence_mode: ThreadEventPersistenceMode::Limited,
            })
            .await
            .expect("resume archived live thread");

        let thread = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    name: Some(Some("Live archived title".to_string())),
                    ..Default::default()
                },
                include_archived: true,
            })
            .await
            .expect("set archived thread name");

        assert!(thread.archived_at.is_some());
        assert!(
            runtime
                .get_thread(thread_id)
                .await
                .expect("get metadata")
                .expect("metadata")
                .archived_at
                .is_some()
        );
    }

    fn test_thread_metadata() -> ThreadPersistenceMetadata {
        ThreadPersistenceMetadata {
            cwd: Some(std::env::current_dir().expect("cwd")),
            model_provider: "test-provider".to_string(),
            memory_mode: ThreadMemoryMode::Enabled,
        }
    }

    fn last_rollout_item(path: &std::path::Path) -> Value {
        let last_line = std::fs::read_to_string(path)
            .expect("read rollout")
            .lines()
            .last()
            .expect("last line")
            .to_string();
        serde_json::from_str(&last_line).expect("json line")
    }
}
