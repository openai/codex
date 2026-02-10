mod extract;

use codex_otel::OtelManager;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use extract::extract_stage_one_output;
use futures::StreamExt;
use tracing::warn;

use super::super::MAX_ROLLOUTS_PER_STARTUP;
use super::super::PHASE_ONE_CONCURRENCY_LIMIT;
use super::super::PHASE_ONE_JOB_LEASE_SECONDS;
use super::super::PHASE_ONE_JOB_RETRY_DELAY_SECONDS;
use super::super::PHASE_ONE_MAX_ROLLOUT_AGE_DAYS;
use super::super::selection::select_rollout_candidates_from_db;
use super::super::types::RolloutCandidate;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::memories::layout::memory_scope_key_for_cwd;
use crate::memories::scope::MEMORY_SCOPE_KEY_USER;
use crate::memories::scope::MEMORY_SCOPE_KIND_CWD;
use crate::memories::scope::MEMORY_SCOPE_KIND_USER;
use std::sync::Arc;

/// Result counters for startup phase-1 extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PhaseOneRunResult {
    /// Number of rollout candidates that were successfully claimed.
    pub(super) claimed_candidate_count: usize,
    /// Number of scope refresh/enqueue operations performed.
    pub(super) touched_scope_count: usize,
}

#[derive(Clone, Debug)]
pub(super) struct ClaimedStageOneCandidate {
    pub(super) candidate: RolloutCandidate,
    pub(super) ownership_token: String,
}

#[derive(Clone)]
struct StageOneRequestContext {
    model_info: ModelInfo,
    otel_manager: OtelManager,
    reasoning_effort: Option<ReasoningEffortConfig>,
    reasoning_summary: ReasoningSummaryConfig,
    turn_metadata_header: Option<String>,
}

impl StageOneRequestContext {
    fn from_turn_context(turn_context: &TurnContext, turn_metadata_header: Option<String>) -> Self {
        Self {
            model_info: turn_context.model_info.clone(),
            otel_manager: turn_context.otel_manager.clone(),
            reasoning_effort: turn_context.reasoning_effort,
            reasoning_summary: turn_context.reasoning_summary,
            turn_metadata_header,
        }
    }
}

/// Runs startup phase 1:
///
/// 1. Select rollout candidates from thread metadata.
/// 2. Claim stage-1 jobs per thread.
/// 3. Execute stage-1 extraction requests in parallel.
/// 4. Persist stage-1 outputs and enqueue consolidation jobs.
pub(super) async fn run_phase_one(
    session: &Arc<Session>,
    thread_items: &[codex_state::ThreadMetadata],
) -> PhaseOneRunResult {
    let selection_candidates = select_rollout_candidates_from_db(
        thread_items,
        session.conversation_id,
        super::PHASE_ONE_THREAD_SCAN_LIMIT,
        PHASE_ONE_MAX_ROLLOUT_AGE_DAYS,
    );
    let claimed_candidates = claim_stage_one_candidates(
        session.as_ref(),
        selection_candidates,
        MAX_ROLLOUTS_PER_STARTUP,
    )
    .await;

    if claimed_candidates.is_empty() {
        return PhaseOneRunResult {
            claimed_candidate_count: 0,
            touched_scope_count: 0,
        };
    }

    let turn_context = session.new_default_turn().await;
    let stage_one_context = StageOneRequestContext::from_turn_context(
        turn_context.as_ref(),
        turn_context.resolve_turn_metadata_header().await,
    );

    let touched_scope_count =
        futures::stream::iter(claimed_candidates.iter().cloned())
            .map(|claimed_candidate| {
                let session = Arc::clone(session);
                let stage_one_context = stage_one_context.clone();
                async move {
                    process_memory_candidate(session, claimed_candidate, stage_one_context).await
                }
            })
            .buffer_unordered(PHASE_ONE_CONCURRENCY_LIMIT)
            .collect::<Vec<usize>>()
            .await
            .into_iter()
            .sum::<usize>();

    PhaseOneRunResult {
        claimed_candidate_count: claimed_candidates.len(),
        touched_scope_count,
    }
}

async fn claim_stage_one_candidates(
    session: &Session,
    candidates: Vec<RolloutCandidate>,
    max_claimed_candidates: usize,
) -> Vec<ClaimedStageOneCandidate> {
    if max_claimed_candidates == 0 {
        return Vec::new();
    }

    let Some(state_db) = session.services.state_db.as_deref() else {
        return Vec::new();
    };

    let mut claimed = Vec::new();
    for candidate in candidates {
        if claimed.len() >= max_claimed_candidates {
            break;
        }

        let claim = match state_db
            .try_claim_stage1_job(
                candidate.thread_id,
                session.conversation_id,
                candidate.source_updated_at,
                PHASE_ONE_JOB_LEASE_SECONDS,
            )
            .await
        {
            Ok(claim) => claim,
            Err(err) => {
                warn!(
                    "state db try_claim_stage1_job failed for rollout {}: {err}",
                    candidate.rollout_path.display()
                );
                continue;
            }
        };

        if let codex_state::Stage1JobClaimOutcome::Claimed { ownership_token } = claim {
            claimed.push(ClaimedStageOneCandidate {
                candidate,
                ownership_token,
            });
        }
    }

    claimed
}

async fn process_memory_candidate(
    session: Arc<Session>,
    claimed_candidate: ClaimedStageOneCandidate,
    stage_one_context: StageOneRequestContext,
) -> usize {
    let candidate = claimed_candidate.candidate;

    let stage_one_output =
        match extract_stage_one_output(session.as_ref(), &candidate, &stage_one_context).await {
            Ok(output) => output,
            Err(reason) => {
                if let Some(state_db) = session.services.state_db.as_deref() {
                    let _ = state_db
                        .mark_stage1_job_failed(
                            candidate.thread_id,
                            &claimed_candidate.ownership_token,
                            reason,
                            PHASE_ONE_JOB_RETRY_DELAY_SECONDS,
                        )
                        .await;
                }
                return 0;
            }
        };

    let Some(state_db) = session.services.state_db.as_deref() else {
        return 0;
    };

    if !state_db
        .mark_stage1_job_succeeded(
            candidate.thread_id,
            &claimed_candidate.ownership_token,
            candidate.source_updated_at,
            &stage_one_output.raw_memory,
            &stage_one_output.summary,
        )
        .await
        .unwrap_or(false)
    {
        return 0;
    }

    let mut touched_scope_count = 0;
    let cwd_scope_key = memory_scope_key_for_cwd(&candidate.cwd);

    if let Err(err) = state_db
        .enqueue_scope_consolidation(
            MEMORY_SCOPE_KIND_CWD,
            &cwd_scope_key,
            candidate.source_updated_at,
        )
        .await
    {
        warn!(
            "failed enqueueing scope consolidation for scope {}:{}: {err}",
            MEMORY_SCOPE_KIND_CWD, cwd_scope_key
        );
    } else {
        touched_scope_count += 1;
    }

    if let Err(err) = state_db
        .enqueue_scope_consolidation(
            MEMORY_SCOPE_KIND_USER,
            MEMORY_SCOPE_KEY_USER,
            candidate.source_updated_at,
        )
        .await
    {
        warn!(
            "failed enqueueing scope consolidation for scope {}:{}: {err}",
            MEMORY_SCOPE_KIND_USER, MEMORY_SCOPE_KEY_USER
        );
    } else {
        touched_scope_count += 1;
    }

    touched_scope_count
}
