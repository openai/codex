use std::sync::Arc;

use codex_async_utils::OrCancelExt;
use codex_extension_api::TurnStartPhase;
use tokio_util::sync::CancellationToken;

use crate::session::TurnInput;
use crate::session::session::Session;
use crate::session::turn::McpStartupRequirements;
use crate::session::turn::run_hooks_and_record_inputs;
use crate::session::turn::run_turn;
use crate::session::turn_context::TurnContext;
use crate::session_startup_prewarm::SessionStartupPrewarmResolution;
use crate::state::TaskKind;
use codex_thread_store::PersistContext;
use tracing::Instrument;
use tracing::trace_span;

use super::SessionTask;
use super::SessionTaskResult;

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
        let run_turn_span = trace_span!("run_turn");
        // Regular turns emit `TurnStarted` inline so first-turn lifecycle does
        // not wait on startup prewarm resolution.
        let prewarmed_client_session = async {
            sess.emit_turn_started(&ctx).await;
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
            let preparation = sess
                .emit_turn_start_lifecycle(
                    &ctx,
                    /*token_usage_at_turn_start*/ None,
                    TurnStartPhase::RegularTaskStart,
                )
                .or_cancel(&cancellation_token)
                .await;
            // Even cancelled discovery may have cleared the previous account's catalog.
            if prepares_mcp {
                sess.request_mcp_runtime_reprojection();
            }
            if preparation.is_err() {
                return SessionStartupPrewarmResolution::Cancelled;
            }
            sess.set_server_reasoning_included(/*included*/ false).await;
            sess.consume_startup_prewarm_for_regular_turn(&cancellation_token)
                .await
        }
        .instrument(trace_span!("regular_task.prepare_run_turn"))
        .await;
        let prewarmed_client_session = match prewarmed_client_session {
            SessionStartupPrewarmResolution::Cancelled => {
                run_hooks_and_record_inputs(
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
        let mut next_input = input;
        let mut prewarmed_client_session = prewarmed_client_session;
        let mut mcp_startup_requirements = McpStartupRequirements::default();
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
            if ctx.terminal_error.lock().await.is_some() {
                return Ok(last_agent_message);
            }
            if !sess.input_queue.has_pending_input(&sess.active_turn).await {
                return Ok(last_agent_message);
            }
            next_input = Vec::new();
        }
    }
}
