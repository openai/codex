//! Applies extension-requested interruption only if the selected turn is still active.
//! Conditional interruption preserves queued input and acknowledges before joining the task.

use super::session::Session;
use crate::codex_thread::CodexThread;
use codex_extension_api::ThreadIdleCause;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::TurnAbortReason;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::TryRecvError;

#[cfg(test)]
#[path = "extension_interruption_tests.rs"]
mod tests;

impl CodexThread {
    /// Interrupts the named active turn unless it has queued input.
    /// Returns the decision before joining cancellation, so tool callbacks can await it.
    /// Like [`Op::Interrupt`], this does not notify idle contributors; the caller owns wakeup.
    pub async fn interrupt_if_no_pending_input(&self, turn_id: &str) -> CodexResult<bool> {
        let cancellation_token = {
            let active = self.session.active_turn.lock().await;
            let Some(task) = active.as_ref().and_then(|turn| turn.task.as_ref()) else {
                return Ok(false);
            };
            if task.turn_context.sub_id != turn_id {
                return Ok(false);
            }
            task.cancellation_token.clone()
        };
        let (reply, mut result) = oneshot::channel();
        tokio::select! {
            biased;
            result = async {
                self.submit(Op::InterruptIfNoPendingInput {
                    turn_id: turn_id.to_owned(),
                    reply,
                }).await?;
                (&mut result).await.map_err(|_| CodexErr::InternalAgentDied)
            } => result,
            // An earlier interrupt may already be waiting for this tool callback to finish.
            // Cancellation can also race with a decision sent by this request.
            _ = cancellation_token.cancelled() => match result.try_recv() {
                Ok(decision) => Ok(decision),
                Err(TryRecvError::Empty | TryRecvError::Closed) => Ok(false),
            },
        }
    }
}

impl Session {
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the turn identity and pending input check must be atomic with taking the task"
    )]
    pub(crate) async fn interrupt_turn_if_no_pending_input(
        self: &Arc<Self>,
        turn_id: &str,
        reply: oneshot::Sender<bool>,
    ) {
        let active_turn = {
            let mut active = self.active_turn.lock().await;
            let Some(turn) = active.as_ref().filter(|turn| {
                turn.task
                    .as_ref()
                    .is_some_and(|task| task.turn_context.sub_id == turn_id)
            }) else {
                let _ = reply.send(false);
                return;
            };
            if !turn.turn_state.lock().await.pending_input.is_empty()
                || self.input_queue.has_pending_mailbox_items().await
                || reply.is_closed()
            {
                let _ = reply.send(false);
                return;
            }
            self.mark_interrupted();
            active.take()
        };
        // The caller may be inside the task that cancellation must join.
        let _ = reply.send(active_turn.is_some());
        if let Some(active_turn) = active_turn {
            self.finish_turn_abort(active_turn, TurnAbortReason::Interrupted)
                .await;
        }
    }

    pub(crate) async fn interrupt_turn_with_warning(
        self: &Arc<Self>,
        turn_id: &str,
        warning: EventMsg,
    ) {
        let Some(turn) = self.turn_context_for_sub_id(turn_id).await else {
            return;
        };
        self.send_event(turn.as_ref(), warning).await;
        let runtime = self.services.runtime_handle.clone();
        let session = Arc::clone(self);
        let turn_id = turn_id.to_owned();
        drop(runtime.spawn(async move {
            if session
                .abort_turn_if_active(&turn_id, TurnAbortReason::Interrupted)
                .await
            {
                // Extension aborts bypass normal task completion; user interrupts do not.
                session
                    .emit_thread_idle_lifecycle_if_idle(ThreadIdleCause::Interrupted)
                    .await;
            }
        }));
    }
}
