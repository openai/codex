use std::sync::Arc;

use crate::session::turn_context::TurnContext;
use crate::state::TaskKind;
use crate::tasks::SessionTask;
use crate::tasks::SessionTaskContext;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::UndoCompletedEvent;
use codex_protocol::protocol::UndoStartedEvent;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;

pub(crate) struct UndoTask;

impl UndoTask {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl SessionTask for UndoTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    fn span_name(&self) -> &'static str {
        "session_task.undo"
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        _input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        session
            .session
            .services
            .session_telemetry
            .counter("codex.task.undo", /*inc*/ 1, &[]);
        let sess = session.clone_session();
        sess.send_event(
            ctx.as_ref(),
            EventMsg::UndoStarted(UndoStartedEvent {
                message: Some("Undo in progress...".to_string()),
            }),
        )
        .await;

        if cancellation_token.is_cancelled() {
            sess.send_event(
                ctx.as_ref(),
                EventMsg::UndoCompleted(UndoCompletedEvent {
                    success: false,
                    message: Some("Undo cancelled.".to_string()),
                }),
            )
            .await;
            return None;
        }

        let completed = UndoCompletedEvent {
            success: false,
            message: Some("Undo is no longer available.".to_string()),
        };

        sess.send_event(ctx.as_ref(), EventMsg::UndoCompleted(completed))
            .await;
        None
    }
}
