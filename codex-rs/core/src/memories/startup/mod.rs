mod dispatch;
mod extract;
mod watch;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::error::Result as CodexResult;
use crate::features::Feature;
use crate::memories::layout::migrate_legacy_user_memory_root_if_needed;
use crate::rollout::INTERACTIVE_SESSION_SOURCES;
use codex_otel::OtelManager;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::SessionSource;
use futures::StreamExt;
use serde_json::Value;
use std::sync::Arc;
use tracing::info;
use tracing::warn;

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
    if let Err(err) = migrate_legacy_user_memory_root_if_needed(&config.codex_home).await {
        warn!("failed migrating legacy shared memory root: {err}");
    }

    let Some(state_db) = session.services.state_db.as_deref() else {
        warn!("state db unavailable for memories startup pipeline; skipping");
        return Ok(());
    };

    let allowed_sources = INTERACTIVE_SESSION_SOURCES
        .iter()
        .map(|value| match serde_json::to_value(value) {
            Ok(Value::String(s)) => s,
            Ok(other) => other.to_string(),
            Err(_) => String::new(),
        })
        .collect::<Vec<_>>();

    let claimed_candidates = match state_db
        .claim_stage1_jobs_for_startup(
            session.conversation_id,
            codex_state::Stage1StartupClaimParams {
                scan_limit: PHASE_ONE_THREAD_SCAN_LIMIT,
                max_claimed: super::MAX_ROLLOUTS_PER_STARTUP,
                max_age_days: super::PHASE_ONE_MAX_ROLLOUT_AGE_DAYS,
                min_rollout_idle_hours: super::PHASE_ONE_MIN_ROLLOUT_IDLE_HOURS,
                allowed_sources: allowed_sources.as_slice(),
                lease_seconds: super::PHASE_ONE_JOB_LEASE_SECONDS,
            },
        )
        .await
    {
        Ok(claims) => claims,
        Err(err) => {
            warn!("state db claim_stage1_jobs_for_startup failed during memories startup: {err}");
            Vec::new()
        }
    };

    let claimed_count = claimed_candidates.len();
    let mut succeeded_count = 0;
    if claimed_count > 0 {
        let turn_context = session.new_default_turn().await;
        let stage_one_context = StageOneRequestContext::from_turn_context(
            turn_context.as_ref(),
            turn_context.resolve_turn_metadata_header().await,
        );

        succeeded_count = futures::stream::iter(claimed_candidates.into_iter())
            .map(|claim| {
                let session = Arc::clone(session);
                let stage_one_context = stage_one_context.clone();
                async move {
                    let thread = claim.thread;
                    let stage_one_output = match extract::extract_stage_one_output(
                        session.as_ref(),
                        &thread.rollout_path,
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
                                        super::PHASE_ONE_JOB_RETRY_DELAY_SECONDS,
                                    )
                                    .await;
                            }
                            return false;
                        }
                    };

                    let Some(state_db) = session.services.state_db.as_deref() else {
                        return false;
                    };

                    state_db
                        .mark_stage1_job_succeeded(
                            thread.id,
                            &claim.ownership_token,
                            thread.updated_at.timestamp(),
                            &stage_one_output.raw_memory,
                            &stage_one_output.rollout_summary,
                        )
                        .await
                        .unwrap_or(false)
                }
            })
            .buffer_unordered(super::PHASE_ONE_CONCURRENCY_LIMIT)
            .collect::<Vec<bool>>()
            .await
            .into_iter()
            .filter(|ok| *ok)
            .count();
    }

    info!(
        "memory stage-1 extraction complete: {} job(s) claimed, {} succeeded",
        claimed_count, succeeded_count
    );

    let consolidation_job_count = run_consolidation_dispatch(session, config).await;
    info!(
        "memory consolidation dispatch complete: {} job(s) scheduled",
        consolidation_job_count
    );

    Ok(())
}

async fn run_consolidation_dispatch(session: &Arc<Session>, config: Arc<Config>) -> usize {
    usize::from(dispatch::run_global_memory_consolidation(session, config).await)
}
