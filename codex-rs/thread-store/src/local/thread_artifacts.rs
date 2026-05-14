use std::collections::HashMap;
use std::path::Path;

use super::LocalThreadStore;
use super::live_writer;
use super::update_thread_metadata::ResolvedRolloutPath;
use super::update_thread_metadata::resolve_rollout_path;
use crate::CreateThreadArtifactParams;
use crate::ListThreadArtifactsParams;
use crate::NewThreadArtifact;
use crate::StoredThreadArtifact;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadArtifact as RolloutThreadArtifact;
use codex_rollout::RolloutRecorder;
use codex_rollout::append_rollout_item_to_path;
use codex_rollout::read_session_meta_line;

pub(super) async fn list_thread_artifacts(
    store: &LocalThreadStore,
    params: ListThreadArtifactsParams,
) -> ThreadStoreResult<Vec<StoredThreadArtifact>> {
    let state_db = state_db(store, params.thread_id).await?;
    state_db
        .get_thread(params.thread_id)
        .await
        .map(|thread| {
            thread
                .map(|metadata| metadata.artifacts)
                .unwrap_or_default()
                .into_iter()
                .map(stored_artifact_from_state)
                .collect()
        })
        .map_err(|err| ThreadStoreError::Internal {
            message: format!(
                "failed to list thread artifacts for {}: {err}",
                params.thread_id
            ),
        })
}

pub(super) async fn create_thread_artifact(
    store: &LocalThreadStore,
    params: CreateThreadArtifactParams,
) -> ThreadStoreResult<StoredThreadArtifact> {
    let resolved_rollout_path = writable_rollout_path(store, params.thread_id).await?;
    let (artifact_type, payload) = state_artifact_kind(params.artifact);
    let artifact_id = uuid::Uuid::now_v7().to_string();
    let artifact = RolloutThreadArtifact {
        created_at: Utc::now().timestamp(),
        artifact_type,
        payload,
    };
    write_rollout_thread_artifacts(
        &resolved_rollout_path,
        params.thread_id,
        HashMap::from([(artifact_id.clone(), Some(artifact.clone()))]),
    )
    .await?;
    let stored_artifact = stored_artifact_from_rollout(artifact_id, artifact);
    append_state_thread_artifact(store, params.thread_id, &stored_artifact).await?;
    Ok(stored_artifact)
}

async fn state_db(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<codex_rollout::StateDbHandle> {
    store
        .state_db()
        .await
        .ok_or_else(|| ThreadStoreError::Internal {
            message: format!("sqlite state db unavailable for thread {thread_id}"),
        })
}

async fn append_state_thread_artifact(
    store: &LocalThreadStore,
    thread_id: ThreadId,
    artifact: &StoredThreadArtifact,
) -> ThreadStoreResult<()> {
    let state_db = state_db(store, thread_id).await?;
    let created_at =
        chrono::DateTime::<Utc>::from_timestamp(artifact.created_at, 0).ok_or_else(|| {
            ThreadStoreError::Internal {
                message: format!(
                    "failed to cache thread artifact {} for {thread_id}: invalid timestamp {}",
                    artifact.id, artifact.created_at
                ),
            }
        })?;
    let updated = state_db
        .append_thread_artifact(codex_state::ThreadArtifact {
            thread_id,
            id: artifact.id.clone(),
            created_at,
            artifact_type: artifact.artifact_type.clone(),
            payload: artifact.payload.clone(),
        })
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to cache thread artifact for {thread_id}: {err}"),
        })?;
    if !updated {
        return Err(ThreadStoreError::ThreadNotFound { thread_id });
    }
    Ok(())
}

pub(super) fn stored_artifact_from_state(
    artifact: codex_state::ThreadArtifact,
) -> StoredThreadArtifact {
    StoredThreadArtifact {
        id: artifact.id,
        created_at: artifact.created_at.timestamp(),
        artifact_type: artifact.artifact_type,
        payload: artifact.payload,
    }
}

pub(super) fn stored_artifact_from_rollout(
    id: String,
    artifact: RolloutThreadArtifact,
) -> StoredThreadArtifact {
    StoredThreadArtifact {
        id,
        created_at: artifact.created_at,
        artifact_type: artifact.artifact_type,
        payload: artifact.payload,
    }
}

fn state_artifact_kind(artifact: NewThreadArtifact) -> (String, serde_json::Value) {
    (artifact.artifact_type, artifact.payload)
}

async fn writable_rollout_path(
    store: &LocalThreadStore,
    thread_id: ThreadId,
) -> ThreadStoreResult<ResolvedRolloutPath> {
    if live_writer::rollout_path(store, thread_id).await.is_ok() {
        live_writer::persist_thread(store, thread_id).await?;
    }
    resolve_rollout_path(store, thread_id, /*include_archived*/ false)
        .await
        .map_err(|err| match err {
            ThreadStoreError::InvalidRequest { .. } => {
                ThreadStoreError::ThreadNotFound { thread_id }
            }
            other => other,
        })
}

pub(super) async fn rollout_thread_artifacts(
    rollout_path: &Path,
    thread_id: ThreadId,
) -> ThreadStoreResult<Vec<StoredThreadArtifact>> {
    let (items, _, _) = RolloutRecorder::load_rollout_items(rollout_path)
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!(
                "failed to read thread artifacts for {thread_id} from {}: {err}",
                rollout_path.display()
            ),
        })?;
    let mut artifacts = Vec::<StoredThreadArtifact>::new();
    for item in items {
        let RolloutItem::SessionMeta(meta_line) = item else {
            continue;
        };
        if meta_line.meta.id != thread_id {
            continue;
        }
        for (id, artifact) in meta_line.artifacts {
            match artifact {
                Some(artifact) => {
                    let updated_artifact = stored_artifact_from_rollout(id.clone(), artifact);
                    if let Some(existing) = artifacts.iter_mut().find(|existing| existing.id == id)
                    {
                        *existing = updated_artifact;
                    } else {
                        artifacts.push(updated_artifact);
                    }
                }
                None => artifacts.retain(|artifact| artifact.id != id),
            }
        }
    }
    Ok(artifacts)
}

async fn write_rollout_thread_artifacts(
    resolved_rollout_path: &ResolvedRolloutPath,
    thread_id: ThreadId,
    artifacts: HashMap<String, Option<RolloutThreadArtifact>>,
) -> ThreadStoreResult<()> {
    let mut session_meta = read_session_meta_line(resolved_rollout_path.path.as_path())
        .await
        .map_err(|err| ThreadStoreError::Internal {
            message: format!("failed to set thread artifacts for {thread_id}: {err}"),
        })?;
    if session_meta.meta.id != thread_id {
        return Err(ThreadStoreError::Internal {
            message: format!(
                "failed to set thread artifacts: rollout session metadata id mismatch: expected {thread_id}, found {}",
                session_meta.meta.id
            ),
        });
    }
    session_meta.git = None;
    session_meta.artifacts = artifacts;
    append_rollout_item_to_path(
        resolved_rollout_path.path.as_path(),
        &RolloutItem::SessionMeta(session_meta),
    )
    .await
    .map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to set thread artifacts for {thread_id}: {err}"),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ThreadStore;
    use crate::local::test_support::test_config;
    use tempfile::TempDir;

    #[tokio::test]
    async fn create_thread_artifact_returns_thread_not_found_for_missing_thread() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let runtime = codex_state::StateRuntime::init(
            config.sqlite_home.clone(),
            config.default_model_provider_id.clone(),
        )
        .await
        .expect("state db should initialize");
        let store = LocalThreadStore::new(config, Some(runtime));
        let thread_id =
            ThreadId::from_string("00000000-0000-0000-0000-000000000123").expect("valid thread id");

        let err = store
            .create_thread_artifact(CreateThreadArtifactParams {
                thread_id,
                artifact: NewThreadArtifact {
                    artifact_type: "github/pull_request".to_string(),
                    payload: serde_json::json!({ "number": 123 }),
                },
            })
            .await
            .expect_err("missing thread should fail");

        assert!(matches!(
            err,
            ThreadStoreError::ThreadNotFound { thread_id: missing_thread_id }
                if missing_thread_id == thread_id
        ));
    }
}
