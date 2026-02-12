mod dispatch;
mod extract;
mod phase2;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::error::Result as CodexResult;
use crate::features::Feature;
use crate::memories::metrics;
use crate::memories::phase_one;
use crate::rollout::INTERACTIVE_SESSION_SOURCES;
use codex_otel::OtelManager;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::SessionSource;
use futures::StreamExt;
use std::sync::Arc;
use tracing::info;
use tracing::warn;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PhaseOneJobOutcome {
    SucceededWithOutput,
    SucceededNoOutput,
    Failed,
}

pub(super) const PHASE_ONE_THREAD_SCAN_LIMIT: usize = 5_000;

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

/// Starts the asynchronous startup memory pipeline for an eligible root session.
///
/// The pipeline is skipped for ephemeral sessions, disabled feature flags, and
/// subagent sessions.
pub(crate) fn start_memories_startup_task(
    session: &Arc<Session>,
    config: Arc<Config>,
    source: &SessionSource,
) {
    if config.ephemeral
        || !config.features.enabled(Feature::MemoryTool)
        || matches!(source, SessionSource::SubAgent(_))
    {
        return;
    }

    let weak_session = Arc::downgrade(session);
    tokio::spawn(async move {
        let Some(session) = weak_session.upgrade() else {
            return;
        };
        if let Err(err) = run_memories_startup_pipeline(&session, config).await {
            warn!("memories startup pipeline failed: {err}");
        }
    });
}

/// Runs the startup memory pipeline.
///
/// Phase 1 selects rollout candidates, performs stage-1 extraction requests in
/// parallel, persists stage-1 outputs, and enqueues consolidation work.
///
/// Phase 2 claims a global consolidation lock and spawns one consolidation agent.
pub(super) async fn run_memories_startup_pipeline(
    session: &Arc<Session>,
    config: Arc<Config>,
) -> CodexResult<()> {
    let otel_manager = &session.services.otel_manager;
    let Some(state_db) = session.services.state_db.as_deref() else {
        warn!("state db unavailable for memories startup pipeline; skipping");
        otel_manager.counter(
            metrics::MEMORY_PHASE_ONE_JOBS,
            1,
            &[("status", "skipped_state_db_unavailable")],
        );
        otel_manager.counter(
            metrics::MEMORY_PHASE_ONE_JOBS,
            1,
            &[("status", "skipped_state_db_unavailable")],
        );
        return Ok(());
    };

    let allowed_sources = INTERACTIVE_SESSION_SOURCES
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    let claimed_candidates = match state_db
        .claim_stage1_jobs_for_startup(
            session.conversation_id,
            codex_state::Stage1StartupClaimParams {
                scan_limit: PHASE_ONE_THREAD_SCAN_LIMIT,
                max_claimed: phase_one::MAX_ROLLOUTS_PER_STARTUP,
                max_age_days: phase_one::MAX_ROLLOUT_AGE_DAYS,
                min_rollout_idle_hours: phase_one::MIN_ROLLOUT_IDLE_HOURS,
                allowed_sources: allowed_sources.as_slice(),
                lease_seconds: phase_one::JOB_LEASE_SECONDS,
            },
        )
        .await
    {
        Ok(claims) => claims,
        Err(err) => {
            warn!("state db claim_stage1_jobs_for_startup failed during memories startup: {err}");
            otel_manager.counter(
                metrics::MEMORY_PHASE_ONE_JOBS,
                1,
                &[("status", "failed_claim")],
            );
            Vec::new()
        }
    };

    let claimed_count = claimed_candidates.len();
    if claimed_count == 0 {
        otel_manager.counter(
            metrics::MEMORY_PHASE_ONE_JOBS,
            1,
            &[("status", "skipped_no_candidates")],
        );
    }
    let mut phase_one_outcomes = Vec::new();
    if claimed_count > 0 {
        let turn_context = session.new_default_turn().await;
        let stage_one_context = StageOneRequestContext::from_turn_context(
            turn_context.as_ref(),
            turn_context.resolve_turn_metadata_header().await,
        );

        phase_one_outcomes = futures::stream::iter(claimed_candidates.into_iter())
            .map(|claim| {
                let session = Arc::clone(session);
                let stage_one_context = stage_one_context.clone();
                async move {
                    let thread = claim.thread;
                    let stage_one_output = match extract::extract_stage_one_output(
                        session.as_ref(),
                        &thread.rollout_path,
                        &thread.cwd,
                        &stage_one_context,
                    )
                    .await
                    {
                        Ok(output) => output,
                        Err(reason) => {
                            if let Some(state_db) = session.services.state_db.as_deref() {
                                let _ = state_db
                                    .mark_stage1_job_failed(
                                        thread.id,
                                        &claim.ownership_token,
                                        reason,
                                        phase_one::JOB_RETRY_DELAY_SECONDS,
                                    )
                                    .await;
                            }
                            return PhaseOneJobOutcome::Failed;
                        }
                    };

                    let Some(state_db) = session.services.state_db.as_deref() else {
                        return PhaseOneJobOutcome::Failed;
                    };

                    if stage_one_output.raw_memory.is_empty()
                        && stage_one_output.rollout_summary.is_empty()
                    {
                        return if state_db
                            .mark_stage1_job_succeeded_no_output(thread.id, &claim.ownership_token)
                            .await
                            .unwrap_or(false)
                        {
                            PhaseOneJobOutcome::SucceededNoOutput
                        } else {
                            PhaseOneJobOutcome::Failed
                        };
                    }

                    if state_db
                        .mark_stage1_job_succeeded(
                            thread.id,
                            &claim.ownership_token,
                            thread.updated_at.timestamp(),
                            &stage_one_output.raw_memory,
                            &stage_one_output.rollout_summary,
                        )
                        .await
                        .unwrap_or(false)
                    {
                        PhaseOneJobOutcome::SucceededWithOutput
                    } else {
                        PhaseOneJobOutcome::Failed
                    }
                }
            })
            .buffer_unordered(phase_one::CONCURRENCY_LIMIT)
            .collect::<Vec<PhaseOneJobOutcome>>()
            .await;
    }

    let succeeded_with_output_count = phase_one_outcomes
        .iter()
        .filter(|outcome| matches!(outcome, PhaseOneJobOutcome::SucceededWithOutput))
        .count();
    let succeeded_no_output_count = phase_one_outcomes
        .iter()
        .filter(|outcome| matches!(outcome, PhaseOneJobOutcome::SucceededNoOutput))
        .count();
    let failed_count = phase_one_outcomes
        .iter()
        .filter(|outcome| matches!(outcome, PhaseOneJobOutcome::Failed))
        .count();
    let succeeded_count = succeeded_with_output_count + succeeded_no_output_count;

    if claimed_count > 0 {
        otel_manager.counter(
            metrics::MEMORY_PHASE_ONE_JOBS,
            claimed_count as i64,
            &[("status", "claimed")],
        );
    }
    if succeeded_with_output_count > 0 {
        otel_manager.counter(
            metrics::MEMORY_PHASE_ONE_JOBS,
            succeeded_with_output_count as i64,
            &[("status", "succeeded")],
        );
        otel_manager.counter(
            metrics::MEMORY_PHASE_ONE_OUTPUT,
            succeeded_with_output_count as i64,
            &[],
        );
    }
    if succeeded_no_output_count > 0 {
        otel_manager.counter(
            metrics::MEMORY_PHASE_ONE_JOBS,
            succeeded_no_output_count as i64,
            &[("status", "succeeded_no_output")],
        );
    }
    if failed_count > 0 {
        otel_manager.counter(
            metrics::MEMORY_PHASE_ONE_JOBS,
            failed_count as i64,
            &[("status", "failed")],
        );
    }

    info!(
        "memory stage-1 extraction complete: {} job(s) claimed, {} succeeded ({} with output, {} no output), {} failed",
        claimed_count,
        succeeded_count,
        succeeded_with_output_count,
        succeeded_no_output_count,
        failed_count
    );

    let consolidation_job_count =
        usize::from(dispatch::run_global_memory_consolidation(session, config).await);
    info!(
        "memory consolidation dispatch complete: {} job(s) scheduled",
        consolidation_job_count
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run_memories_startup_pipeline;
    use crate::codex::make_session_and_context;
    use crate::config::test_config;
    use std::sync::Arc;

    #[tokio::test]
    async fn startup_pipeline_is_noop_when_state_db_is_unavailable() {
        let (session, _turn_context) = make_session_and_context().await;
        let session = Arc::new(session);
        let config = Arc::new(test_config());
        run_memories_startup_pipeline(&session, config)
            .await
            .expect("startup pipeline should skip cleanly without state db");
    }
}
