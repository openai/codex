use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::session::TurnInput;
use crate::session::turn::AutoCompactTurnLimiter;
use crate::session::turn::RunTurnResult;
use crate::session::turn::run_turn;
use crate::session::turn_context::TurnContext;
use crate::session_startup_prewarm::SessionStartupPrewarmResolution;
use crate::state::TaskKind;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TurnStartedEvent;
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

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: Vec<TurnInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        let sess = session.clone_session();
        let turn_extension_data = session.turn_extension_data();
        let run_turn_span = trace_span!("run_turn");
        // Regular turns emit `TurnStarted` inline so first-turn lifecycle does
        // not wait on startup prewarm resolution.
        let event = EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: ctx.sub_id.clone(),
            trace_id: ctx.trace_id.clone(),
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
            match turn_result {
                RunTurnResult::Continue(last_agent_message) => {
                    if !sess.input_queue.has_pending_input(&sess.active_turn).await {
                        return last_agent_message;
                    }
                }
                RunTurnResult::Terminal => {
                    let turn_state = {
                        let active_turn = sess.active_turn.lock().await;
                        active_turn
                            .as_ref()
                            .map(|active_turn| Arc::clone(&active_turn.turn_state))
                    };
                    if let Some(turn_state) = turn_state {
                        let pending_input = sess
                            .input_queue
                            .mark_terminal_and_take_pending_for_turn_state(turn_state.as_ref())
                            .await;
                        let mut rejected_user_input = false;
                        let mut response_items_for_next_turn = Vec::new();
                        for pending_input_item in pending_input {
                            match pending_input_item {
                                TurnInput::UserInput(_) => rejected_user_input = true,
                                TurnInput::ResponseInputItem(item) => {
                                    response_items_for_next_turn.push(item);
                                }
                            }
                        }
                        sess.input_queue
                            .queue_response_items_for_next_turn(response_items_for_next_turn)
                            .await;
                        if rejected_user_input {
                            sess.send_event(
                                ctx.as_ref(),
                                EventMsg::Error(ErrorEvent {
                                    message: "Pending user input was not processed because Codex stopped this turn after repeated automatic compactions. Submit it again in a new turn or reduce earlier history before retrying.".to_string(),
                                    codex_error_info: Some(CodexErrorInfo::ContextWindowExceeded),
                                }),
                            )
                            .await;
                        }
                    }
                    return None;
                }
            }
            next_input = Vec::new();
        }
    }
}
