use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::session::turn::AutoCompactTurnLimiter;
use crate::session::turn::RunTurnResult;
use crate::session::turn::run_turn;
use crate::session::turn_context::TurnContext;
use crate::session_startup_prewarm::SessionStartupPrewarmResolution;
use crate::state::TaskKind;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::user_input::UserInput;
use tracing::Instrument;
use tracing::trace_span;

use super::SessionTask;
use super::SessionTaskContext;

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

    fn records_turn_token_usage_on_span(&self) -> bool {
        true
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        let sess = session.clone_session();
        let turn_extension_data = session.turn_extension_data();
        let run_turn_span = trace_span!("run_turn");
        // Regular turns emit `TurnStarted` inline so first-turn lifecycle does
        // not wait on startup prewarm resolution.
        let event = EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: ctx.sub_id.clone(),
            started_at: ctx.turn_timing_state.started_at_unix_secs().await,
            model_context_window: ctx.model_context_window(),
            collaboration_mode_kind: ctx.collaboration_mode.mode,
        });
        sess.send_event(ctx.as_ref(), event).await;
        sess.set_server_reasoning_included(/*included*/ false).await;
        let prewarmed_client_session = match sess
            .consume_startup_prewarm_for_regular_turn(&cancellation_token)
            .await
        {
            SessionStartupPrewarmResolution::Cancelled => return None,
            SessionStartupPrewarmResolution::Unavailable { .. } => None,
            SessionStartupPrewarmResolution::Ready(prewarmed_client_session) => {
                Some(*prewarmed_client_session)
            }
        };
        let mut next_input = input;
        let mut prewarmed_client_session = prewarmed_client_session;
        let mut auto_compact_limiter = AutoCompactTurnLimiter::default();
        loop {
            let turn_result = run_turn(
                Arc::clone(&sess),
                Arc::clone(&ctx),
                Arc::clone(&turn_extension_data),
                &mut auto_compact_limiter,
                next_input,
                prewarmed_client_session.take(),
                cancellation_token.child_token(),
            )
            .instrument(run_turn_span.clone())
            .await;
            let (last_agent_message, can_restart_with_pending_input) = match turn_result {
                RunTurnResult::Continue(last_agent_message) => (last_agent_message, true),
                RunTurnResult::Terminal => {
                    let turn_state = {
                        let active_turn = sess.active_turn.lock().await;
                        active_turn
                            .as_ref()
                            .map(|active_turn| Arc::clone(&active_turn.turn_state))
                    };
                    if let Some(turn_state) = turn_state {
                        turn_state.lock().await.clear_pending_waiters();
                        drop(
                            sess.input_queue
                                .take_pending_input_for_turn_state(turn_state.as_ref())
                                .await,
                        );
                    }
                    (None, false)
                }
            };
            if !can_restart_with_pending_input {
                return last_agent_message;
            }
            if !sess.input_queue.has_pending_input(&sess.active_turn).await {
                return last_agent_message;
            }
            next_input = Vec::new();
        }
    }
}
