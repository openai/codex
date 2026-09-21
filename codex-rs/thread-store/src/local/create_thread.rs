use super::LocalThreadStore;
use crate::CreateThreadParams;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_rollout::RolloutConfig;
use codex_rollout::RolloutRecorder;
use codex_rollout::RolloutRecorderParams;
use codex_utils_absolute_path::AbsolutePathBuf;

pub(super) async fn create_thread(
    store: &LocalThreadStore,
    params: CreateThreadParams,
    writer_lock: super::WriterLockGuard,
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
        sqlite: store.config.sqlite.clone(),
        cwd,
        model_provider_id: params.metadata.model_provider.clone(),
        generate_memories: matches!(params.metadata.memory_mode, ThreadMemoryMode::Enabled),
    };
    RolloutRecorder::new_with_writer_lock(
        &config,
        RolloutRecorderParams::new(
            params.thread_id,
            params.forked_from_id,
            params.parent_thread_id,
            params.source,
            params.thread_source,
            params.originator,
            params.base_instructions,
            params.dynamic_tools,
        )
        .with_creator(params.creator_user_id, params.creator_account_id)
        .with_session_id(params.session_id)
        .with_selected_capability_roots(params.selected_capability_roots)
        .with_runtime_workspace_roots(params.runtime_workspace_roots.map(|roots| {
            roots
                .into_iter()
                .map(AbsolutePathBuf::into_path_buf)
                .collect()
        }))
        .with_multi_agent_version(params.multi_agent_version)
        .with_history_mode(params.history_mode)
        .with_history_base(params.history_base)
        .with_forked_from_ordinal_exclusive(
            params
                .forked_from_id
                .and(params.history_base)
                .map(|base| base.end_ordinal_exclusive),
        )
        .with_subagent_history_start_ordinal(params.subagent_history_start_ordinal)
        .with_initial_window_id(params.initial_window_id),
        writer_lock,
    )
    .await
    .map_err(|err| ThreadStoreError::Internal {
        message: format!("failed to initialize local thread recorder: {err}"),
    })
}
