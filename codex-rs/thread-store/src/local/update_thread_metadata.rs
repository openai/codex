use std::path::Path;
use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GitInfo;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_protocol::protocol::ThreadNameUpdatedEvent;
use codex_rollout::ARCHIVED_SESSIONS_SUBDIR;
use codex_rollout::append_rollout_item_to_path;
use codex_rollout::append_thread_name;
use codex_rollout::find_archived_thread_path_by_id_str;
use codex_rollout::find_thread_path_by_id_str;
use codex_rollout::read_session_meta_line;

use super::LocalThreadStore;
use super::helpers::git_info_from_parts;
use super::live_writer;
use crate::GitInfoPatch;
use crate::ReadThreadParams;
use crate::StoredThread;
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
    let field_count = usize::from(params.patch.name.is_some())
        + usize::from(params.patch.memory_mode.is_some())
        + usize::from(params.patch.git_info.is_some());
    if field_count > 1 {
        return Err(ThreadStoreError::InvalidRequest {
            message: "local thread store applies one metadata field per patch in this slice"
                .to_string(),
        });
    }

    let thread_id = params.thread_id;
    if live_writer::rollout_path(store, thread_id).await.is_ok() {
        live_writer::persist_thread(store, thread_id).await?;
    }
    let resolved_rollout_path =
        resolve_rollout_path(store, thread_id, params.include_archived).await?;
    let git_info = params.patch.git_info;
    if let Some(name) = params.patch.name {
        apply_thread_name(store, resolved_rollout_path.path.as_path(), thread_id, name).await?;
    }
    if let Some(memory_mode) = params.patch.memory_mode {
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
    rollout_path: &Path,
    thread_id: ThreadId,
    name: String,
) -> ThreadStoreResult<()> {
    let item = RolloutItem::EventMsg(EventMsg::ThreadNameUpdated(ThreadNameUpdatedEvent {
        thread_id,
        thread_name: Some(name.clone()),
    }));

    append_rollout_item_to_path(rollout_path, &item)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to set thread name: {err}"),
        })?;
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
    use crate::ResumeThreadParams;
    use crate::ThreadEventPersistenceMode;
    use crate::ThreadMetadataPatch;
    use crate::ThreadPersistenceMetadata;
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
        let path =
            write_session_file(home.path(), "2025-01-03T14-00-00", uuid).expect("session file");

        let thread = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    name: Some("A sharper name".to_string()),
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

        let appended = last_rollout_item(path.as_path());
        assert_eq!(appended["type"], "event_msg");
        assert_eq!(appended["payload"]["type"], "thread_name_updated");
        assert_eq!(appended["payload"]["thread_id"], thread_id.to_string());
        assert_eq!(appended["payload"]["thread_name"], "A sharper name");
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
    async fn update_thread_metadata_rejects_multi_field_patch_without_partial_write() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()), /*state_db*/ None);
        let uuid = Uuid::from_u128(305);
        let thread_id = ThreadId::from_string(&uuid.to_string()).expect("valid thread id");
        let path =
            write_session_file(home.path(), "2025-01-03T15-30-00", uuid).expect("session file");
        let original = std::fs::read_to_string(&path).expect("read rollout");

        let err = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    name: Some("Should not persist".to_string()),
                    memory_mode: Some(ThreadMemoryMode::Disabled),
                    ..Default::default()
                },
                include_archived: false,
            })
            .await
            .expect_err("multi-field patch should fail");

        assert!(matches!(err, ThreadStoreError::InvalidRequest { .. }));
        assert_eq!(
            std::fs::read_to_string(&path).expect("read rollout"),
            original
        );
        let latest_name = codex_rollout::find_thread_name_by_id(home.path(), &thread_id)
            .await
            .expect("find thread name");
        assert_eq!(latest_name, None);
    }

    #[tokio::test]
    async fn update_thread_metadata_keeps_archived_thread_archived_in_sqlite() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let uuid = Uuid::from_u128(306);
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
                    name: Some("Archived title".to_string()),
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
                    name: Some("Live archived title".to_string()),
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
