mod dispatch;
mod extract;
mod phase2;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::error::Result as CodexResult;
use crate::features::Feature;
use crate::rollout::INTERACTIVE_SESSION_SOURCES;
use codex_otel::OtelManager;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::BackgroundEventEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use futures::StreamExt;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
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
    progress_sub_id: Option<String>,
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
        if let Err(err) = run_memories_startup_pipeline(&session, config, progress_sub_id).await {
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
    progress_sub_id: Option<String>,
) -> CodexResult<()> {
    let Some(state_db) = session.services.state_db.as_deref() else {
        warn!("state db unavailable for memories startup pipeline; skipping");
        emit_memory_progress(
            session.as_ref(),
            &progress_sub_id,
            "phase 1 skipped (state db unavailable)",
        )
        .await;
        return Ok(());
    };

    emit_memory_progress(
        session.as_ref(),
        &progress_sub_id,
        "phase 1 scanning candidates",
    )
    .await;

    let allowed_sources = INTERACTIVE_SESSION_SOURCES
        .iter()
        .map(ToString::to_string)
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
    emit_memory_progress(
        session.as_ref(),
        &progress_sub_id,
        format!("phase 1 running (0/{claimed_count} done)"),
    )
    .await;
    let mut succeeded_count = 0;
    if claimed_count > 0 {
        let turn_context = session.new_default_turn().await;
        let stage_one_context = StageOneRequestContext::from_turn_context(
            turn_context.as_ref(),
            turn_context.resolve_turn_metadata_header().await,
        );
        let completed_count = Arc::new(AtomicUsize::new(0));

        succeeded_count = futures::stream::iter(claimed_candidates.into_iter())
            .map(|claim| {
                let session = Arc::clone(session);
                let stage_one_context = stage_one_context.clone();
                let progress_sub_id = progress_sub_id.clone();
                let completed_count = Arc::clone(&completed_count);
                async move {
                    let thread = claim.thread;
                    let job_succeeded = match extract::extract_stage_one_output(
                        session.as_ref(),
                        &thread.rollout_path,
                        &thread.cwd,
                        &stage_one_context,
                    )
                    .await
                    {
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
                            false
                        }
                        Ok(stage_one_output) => {
                            if let Some(state_db) = session.services.state_db.as_deref() {
                                if stage_one_output.raw_memory.is_empty()
                                    && stage_one_output.rollout_summary.is_empty()
                                {
                                    state_db
                                        .mark_stage1_job_succeeded_no_output(
                                            thread.id,
                                            &claim.ownership_token,
                                        )
                                        .await
                                        .unwrap_or(false)
                                } else {
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
                            } else {
                                false
                            }
                        }
                    };

                    let done = completed_count.fetch_add(1, Ordering::Relaxed) + 1;
                    emit_memory_progress(
                        session.as_ref(),
                        &progress_sub_id,
                        format!("phase 1 running ({done}/{claimed_count} done)"),
                    )
                    .await;
                    job_succeeded
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
    emit_memory_progress(
        session.as_ref(),
        &progress_sub_id,
        format!("phase 1 complete ({succeeded_count}/{claimed_count} succeeded)"),
    )
    .await;

    let consolidation_job_count = usize::from(
        dispatch::run_global_memory_consolidation(session, config, &progress_sub_id).await,
    );
    info!(
        "memory consolidation dispatch complete: {} job(s) scheduled",
        consolidation_job_count
    );

    Ok(())
}

pub(super) async fn emit_memory_progress(
    session: &Session,
    progress_sub_id: &Option<String>,
    message: impl Into<String>,
) {
    let Some(sub_id) = progress_sub_id.as_ref() else {
        return;
    };
    session
        .send_event_raw(Event {
            id: sub_id.clone(),
            msg: EventMsg::BackgroundEvent(BackgroundEventEvent {
                message: format!("memory startup: {}", message.into()),
            }),
        })
        .await;
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
        run_memories_startup_pipeline(&session, config, None)
            .await
            .expect("startup pipeline should skip cleanly without state db");
    }
}
