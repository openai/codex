use super::LocalThreadStore;
use crate::CreateThreadParams;
use crate::ThreadEventPersistenceMode;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_rollout::EventPersistenceMode;
use codex_rollout::RolloutConfig;
use codex_rollout::RolloutRecorder;
use codex_rollout::RolloutRecorderParams;

pub(super) async fn create_thread(
    store: &LocalThreadStore,
    params: CreateThreadParams,
) -> ThreadStoreResult<RolloutRecorder> {
    let cwd = params
        .metadata
        .cwd
        .clone()
        .ok_or_else(|| ThreadStoreError::InvalidRequest {
            message: "local thread store requires a cwd".to_string(),
        })?;
    let config = RolloutConfig {
        codex_home: store.config.codex_home.clone(),
        sqlite_home: store.config.sqlite_home.clone(),
        cwd,
        model_provider_id: params.metadata.model_provider.clone(),
        generate_memories: matches!(params.metadata.memory_mode, ThreadMemoryMode::Enabled),
    };
    let state_db_ctx = store.state_db().await;
    let recorder = RolloutRecorder::new(
        &config,
        RolloutRecorderParams::new(
            params.thread_id,
            params.forked_from_id,
            params.source,
            params.base_instructions,
            params.dynamic_tools,
            event_persistence_mode(params.event_persistence_mode),
        ),
        state_db_ctx,
        /*state_builder*/ None,
    )
    .await
    .map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to initialize local thread recorder: {err}"),
    })?;

    Ok(recorder)
}

pub(super) fn event_persistence_mode(mode: ThreadEventPersistenceMode) -> EventPersistenceMode {
    match mode {
        ThreadEventPersistenceMode::Limited => EventPersistenceMode::Limited,
        ThreadEventPersistenceMode::Extended => EventPersistenceMode::Extended,
    }
}
