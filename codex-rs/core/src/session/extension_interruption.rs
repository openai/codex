//! Applies extension-requested interruption only if the selected turn is still active.
//! The caller selects the warning; the host owns task cancellation and idle notification.

use super::session::Session;
use codex_extension_api::ThreadIdleCause;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TurnAbortReason;
use std::sync::Arc;

impl Session {
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
