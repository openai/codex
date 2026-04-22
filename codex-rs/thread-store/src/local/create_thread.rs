use super::LocalThreadStore;
use crate::CreateThreadParams;
use crate::ThreadEventPersistenceMode;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use codex_rollout::EventPersistenceMode;
use codex_rollout::RolloutRecorder;
use codex_rollout::RolloutRecorderParams;

pub(super) async fn create_thread(
    store: &LocalThreadStore,
    params: CreateThreadParams,
) -> ThreadStoreResult<RolloutRecorder> {
    let state_db_ctx = store.state_db().await;
    let recorder = RolloutRecorder::new(
        &store.config,
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
