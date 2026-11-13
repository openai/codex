use crate::McpRefreshReason;
use crate::agent::helper_daybreak::turn::SavedProgramResolution;
use std::sync::Arc;

use codex_async_utils::OrCancelExt;
use codex_extension_api::TurnStartPhase;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use futures::future::BoxFuture;
use tokio_util::sync::CancellationToken;

use crate::client::ModelClientSession;
use crate::session::TurnInput;
use crate::session::session::Session;
use crate::session::startup_prewarm::SessionStartupPrewarmResolution;
use crate::session::turn::McpStartupRequirements;
use crate::session::turn::run_hooks_and_record_inputs;
use crate::session::turn::run_turn;
use crate::session::turn_context::TurnContext;
use crate::state::TaskKind;
use codex_thread_store::PersistContext;
use tracing::Instrument;
use tracing::trace_span;

use super::SessionTask;
use super::SessionTaskResult;

/// The last answer and the MCP requirements needed if a steer reopens sampling.
pub(crate) struct RegularTaskOutput {
    pub(super) last_agent_message: Option<String>,
    pub(super) mcp_startup_requirements: McpStartupRequirements,
}

#[derive(Default)]
pub(crate) struct RegularTask;

impl RegularTask {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl SessionTask for RegularTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    fn span_name(&self) -> &'static str {
        "session_task.turn"
    }

    async fn run(
        self: Arc<Self>,
        sess: Arc<Session>,
        ctx: Arc<TurnContext>,
        input: Vec<TurnInput>,
        cancellation_token: CancellationToken,
    ) -> SessionTaskResult {
        if ctx
            .config
            .features
            .enabled(codex_features::Feature::CriticalPathTiming)
            && !matches!(
                ctx.session_source,
                SessionSource::Internal(InternalSessionSource::MemoryConsolidation)
                    | SessionSource::SubAgent(SubAgentSource::MemoryConsolidation)
            )
        {
            ctx.turn_timing_state.enable_critical_path();
        }
        // Regular turns emit `TurnStarted` inline so first-turn lifecycle does
        // not wait on startup prewarm resolution.
        let prewarmed_client_session = async {
            sess.emit_turn_started(&ctx, TaskKind::Regular).await;
            sess.start_deferred_mcp_reprojection();
            // Regular-start contributors run once, after the task is visible and interruptible.
            let prepares_mcp = sess
                .services
                .extensions
                .turn_lifecycle_contributors()
                .iter()
                .any(|contributor| {
                    contributor.turn_start_phase(&sess.services.thread_extension_data)
                        == TurnStartPhase::RegularTaskStart
                        && contributor.requires_mcp_runtime(&sess.services.thread_extension_data)
                });
            let preparation = async {
                sess.emit_turn_start_lifecycle(
                    &ctx,
                    /*token_usage_at_turn_start*/ None,
                    TurnStartPhase::RegularTaskStart,
                )
                .await;
                if prepares_mcp {
                    // Resource discovery can overlap the fetch. Wait before reprojection.
                    sess.wait_for_apps_tool_refresh().await;
                }
            }
            .or_cancel(&cancellation_token)
            .await;
            // Even cancelled discovery may have cleared the previous account's catalog.
            if prepares_mcp {
                sess.request_mcp_runtime_reprojection(
                    McpRefreshReason::TurnStartContributorPreparedMcp,
                );
            }
            if preparation.is_err() {
                return SessionStartupPrewarmResolution::Cancelled;
            }
            sess.consume_startup_prewarm_for_regular_turn(&cancellation_token)
                .await
        }
        .instrument(trace_span!("regular_task.prepare_run_turn"))
        .await;
        let prewarmed_client_session = match prewarmed_client_session {
            SessionStartupPrewarmResolution::Cancelled => {
                let _ = run_hooks_and_record_inputs(
                    &sess,
                    &ctx,
                    &ctx.capture_current_model_info(),
                    &input,
                    PersistContext::Standard,
                )
                .await;
                return Ok(None);
            }
            SessionStartupPrewarmResolution::Unavailable { .. } => None,
            SessionStartupPrewarmResolution::Ready(prewarmed_client_session) => {
                Some(*prewarmed_client_session)
            }
        };
        if let Some(resolution) = ctx.extension_data.get::<SavedProgramResolution>()
            && let Err(error) = resolution.validate()
        {
            let _ = run_hooks_and_record_inputs(
                &sess,
                &ctx,
                &ctx.capture_current_model_info(),
                &input,
                PersistContext::Standard,
            )
            .await;
            return Err(error);
        }
        Self::run_turns(
            sess,
            ctx,
            input,
            McpStartupRequirements::default(),
            prewarmed_client_session,
            cancellation_token,
        )
        .await
    }
}

impl RegularTask {
    /// Runs model requests for a regular turn, retaining explicit MCP requirements across steers.
    pub(super) fn run_turns(
        sess: Arc<Session>,
        ctx: Arc<TurnContext>,
        mut next_input: Vec<TurnInput>,
        mut mcp_startup_requirements: McpStartupRequirements,
        mut prewarmed_client_session: Option<ModelClientSession>,
        cancellation_token: CancellationToken,
    ) -> BoxFuture<'static, SessionTaskResult> {
        Box::pin(async move {
            let run_turn_span = trace_span!(
                "run_turn",
                conversation.id = %sess.thread_id,
                turn.id = %ctx.sub_id,
            );
            loop {
                let last_agent_message = run_turn(
                    Arc::clone(&sess),
                    Arc::clone(&ctx),
                    next_input,
                    &mut mcp_startup_requirements,
                    prewarmed_client_session.take(),
                    cancellation_token.child_token(),
                )
                .instrument(run_turn_span.clone())
                .await?;
                // Terminal errors are already reported. Let task completion preserve pending
                // input instead of restarting the failed turn for that same input.
                if ctx.terminal_error.lock().await.is_some()
                    || !sess.input_queue.has_pending_input(&sess.active_turn).await
                {
                    return Ok(Some(RegularTaskOutput {
                        last_agent_message,
                        mcp_startup_requirements,
                    }));
                }
                next_input = Vec::new();
            }
        })
    }
}
