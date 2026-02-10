use crate::codex::Session;
use crate::config::Config;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use std::sync::Arc;
use tracing::debug;
use tracing::info;
use tracing::warn;

use super::super::MAX_RAW_MEMORIES_PER_SCOPE;
use super::super::MEMORY_CONSOLIDATION_SUBAGENT_LABEL;
use super::super::PHASE_TWO_JOB_LEASE_SECONDS;
use super::super::PHASE_TWO_JOB_RETRY_DELAY_SECONDS;
use super::super::prompts::build_consolidation_prompt;
use super::super::storage::rebuild_memory_summary_from_memories;
use super::super::storage::sync_raw_memories_from_memories;
use super::super::storage::wipe_consolidation_outputs;
use super::MemoryScopeTarget;
use super::watch::spawn_phase2_completion_task;

pub(super) async fn run_memory_consolidation_for_scope(
    session: Arc<Session>,
    config: Arc<Config>,
    scope: MemoryScopeTarget,
) {
    let Some(state_db) = session.services.state_db.as_deref() else {
        warn!(
            "state db unavailable for scope {}:{}; skipping consolidation",
            scope.scope_kind, scope.scope_key
        );
        return;
    };

    let claim = match state_db
        .try_claim_phase2_job(
            scope.scope_kind,
            &scope.scope_key,
            session.conversation_id,
            PHASE_TWO_JOB_LEASE_SECONDS,
        )
        .await
    {
        Ok(claim) => claim,
        Err(err) => {
            warn!(
                "state db try_claim_phase2_job failed for scope {}:{}: {err}",
                scope.scope_kind, scope.scope_key
            );
            return;
        }
    };
    let (ownership_token, claimed_watermark) = match claim {
        codex_state::Phase2JobClaimOutcome::Claimed {
            ownership_token,
            input_watermark,
        } => (ownership_token, input_watermark),
        codex_state::Phase2JobClaimOutcome::SkippedNotDirty => {
            debug!(
                "memory phase-2 scope not pending (or already up to date); skipping consolidation: {}:{}",
                scope.scope_kind, scope.scope_key
            );
            return;
        }
        codex_state::Phase2JobClaimOutcome::SkippedRunning => {
            debug!(
                "memory phase-2 job already running for scope {}:{}; skipping",
                scope.scope_kind, scope.scope_key
            );
            return;
        }
    };

    let latest_memories = match state_db
        .list_stage1_outputs_for_scope(
            scope.scope_kind,
            &scope.scope_key,
            MAX_RAW_MEMORIES_PER_SCOPE,
        )
        .await
    {
        Ok(memories) => memories,
        Err(err) => {
            warn!(
                "state db list_stage1_outputs_for_scope failed during consolidation for scope {}:{}: {err}",
                scope.scope_kind, scope.scope_key
            );
            let _ = state_db
                .mark_phase2_job_failed(
                    scope.scope_kind,
                    &scope.scope_key,
                    &ownership_token,
                    "failed to read scope stage-1 outputs before consolidation",
                    PHASE_TWO_JOB_RETRY_DELAY_SECONDS,
                )
                .await;
            return;
        }
    };
    if latest_memories.is_empty() {
        debug!(
            "memory phase-2 scope has no stage-1 outputs; skipping consolidation: {}:{}",
            scope.scope_kind, scope.scope_key
        );
        let _ = state_db
            .mark_phase2_job_succeeded(
                scope.scope_kind,
                &scope.scope_key,
                &ownership_token,
                claimed_watermark,
            )
            .await;
        return;
    };

    let materialized_watermark = latest_memories
        .iter()
        .map(|memory| memory.source_updated_at.timestamp())
        .max()
        .unwrap_or(claimed_watermark);

    if let Err(err) = sync_raw_memories_from_memories(&scope.memory_root, &latest_memories).await {
        warn!(
            "failed syncing phase-1 raw memories for scope {}:{}: {err}",
            scope.scope_kind, scope.scope_key
        );
        let _ = state_db
            .mark_phase2_job_failed(
                scope.scope_kind,
                &scope.scope_key,
                &ownership_token,
                "failed syncing phase-1 raw memories",
                PHASE_TWO_JOB_RETRY_DELAY_SECONDS,
            )
            .await;
        return;
    }

    if let Err(err) =
        rebuild_memory_summary_from_memories(&scope.memory_root, &latest_memories).await
    {
        warn!(
            "failed rebuilding memory summary for scope {}:{}: {err}",
            scope.scope_kind, scope.scope_key
        );
        let _ = state_db
            .mark_phase2_job_failed(
                scope.scope_kind,
                &scope.scope_key,
                &ownership_token,
                "failed rebuilding memory summary",
                PHASE_TWO_JOB_RETRY_DELAY_SECONDS,
            )
            .await;
        return;
    }

    if let Err(err) = wipe_consolidation_outputs(&scope.memory_root).await {
        warn!(
            "failed to wipe previous consolidation outputs for scope {}:{}: {err}",
            scope.scope_kind, scope.scope_key
        );
        let _ = state_db
            .mark_phase2_job_failed(
                scope.scope_kind,
                &scope.scope_key,
                &ownership_token,
                "failed to wipe previous consolidation outputs",
                PHASE_TWO_JOB_RETRY_DELAY_SECONDS,
            )
            .await;
        return;
    }

    let prompt = build_consolidation_prompt(&scope.memory_root);
    let input = vec![UserInput::Text {
        text: prompt,
        text_elements: vec![],
    }];
    let mut consolidation_config = config.as_ref().clone();
    consolidation_config.cwd = scope.memory_root.clone();
    let source = SessionSource::SubAgent(SubAgentSource::Other(
        MEMORY_CONSOLIDATION_SUBAGENT_LABEL.to_string(),
    ));

    match session
        .services
        .agent_control
        .spawn_agent(consolidation_config, input, Some(source))
        .await
    {
        Ok(consolidation_agent_id) => {
            info!(
                "memory phase-2 consolidation agent started: scope={} scope_key={} agent_id={}",
                scope.scope_kind, scope.scope_key, consolidation_agent_id
            );
            spawn_phase2_completion_task(
                session.as_ref(),
                scope,
                ownership_token,
                materialized_watermark,
                consolidation_agent_id,
            );
        }
        Err(err) => {
            warn!(
                "failed to spawn memory consolidation agent for scope {}:{}: {err}",
                scope.scope_kind, scope.scope_key
            );
            let _ = state_db
                .mark_phase2_job_failed(
                    scope.scope_kind,
                    &scope.scope_key,
                    &ownership_token,
                    "failed to spawn consolidation agent",
                    PHASE_TWO_JOB_RETRY_DELAY_SECONDS,
                )
                .await;
        }
    }
}
