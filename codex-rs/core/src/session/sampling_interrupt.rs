//! Cancels an opted-in child's tools while it retains its sampled item or installs its summary.

use super::session::Session;
use super::turn_context::TurnContext;
use crate::state::ActiveTurn;
use codex_features::Feature;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::MultiAgentVersion;
use std::ops::ControlFlow;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Owned by the active turn through publication of its terminal event.
pub(crate) struct SamplingInterrupt {
    tools: CancellationToken,
    pub(super) finished: CancellationToken,
}

impl Drop for SamplingInterrupt {
    fn drop(&mut self) {
        self.finished.cancel();
    }
}

impl SamplingInterrupt {
    pub(crate) fn enabled(turn: &TurnContext) -> bool {
        turn.config.multi_agent_v2.interrupt_at_item_boundary
            && turn.multi_agent_version == MultiAgentVersion::V2
            && turn.parent_thread_id.is_some()
    }

    pub(crate) async fn register(
        session: &Session,
        turn: &TurnContext,
        tools: &CancellationToken,
    ) -> CodexResult<()> {
        let mut active = session.active_turn.lock().await;
        let active = matching_turn(&mut active, turn).ok_or(CodexErr::TurnAborted)?;
        if tools.is_cancelled() {
            return Err(CodexErr::TurnAborted);
        }
        active.sampling_interrupt = Some(Self {
            tools: tools.clone(),
            finished: CancellationToken::new(),
        });
        Ok(())
    }

    /// Releases an uninterrupted attempt; returns whether a stop is pending instead.
    pub(crate) async fn unregister(session: &Session, turn: &TurnContext) -> bool {
        let mut active = session.active_turn.lock().await;
        let Some(active) = matching_turn(&mut active, turn) else {
            return false;
        };
        if active
            .sampling_interrupt
            .as_ref()
            .is_some_and(Self::is_requested)
        {
            return true;
        }
        active.sampling_interrupt = None;
        false
    }

    pub(crate) fn is_requested(&self) -> bool {
        self.tools.is_cancelled()
    }

    pub(crate) async fn is_requested_for(session: &Session, turn: &TurnContext) -> bool {
        let mut active = session.active_turn.lock().await;
        matching_turn(&mut active, turn)
            .and_then(|active| active.sampling_interrupt.as_ref())
            .is_some_and(Self::is_requested)
    }

    pub(super) async fn request(session: &Arc<Session>) {
        let action = {
            let active = session.active_turn.lock().await;
            if let Some(ActiveTurn {
                sampling_interrupt: Some(interrupt),
                task,
                ..
            }) = active.as_ref()
            {
                if interrupt.is_requested() {
                    return;
                }
                interrupt.tools.cancel();
                let Some(task) = task else { return };
                task.mcp_elicitation_cancellation.cancel();
                ControlFlow::Continue(
                    task.turn_context
                        .config
                        .features
                        .enabled(Feature::CodeModeInterrupt),
                )
            } else {
                // Claim the hard abort under the same lock that admits a sampling attempt.
                ControlFlow::Break(session.interrupt_task(active))
            }
        };
        match action {
            ControlFlow::Break(abort) => abort.await,
            ControlFlow::Continue(true) => {
                session
                    .services
                    .code_mode_service
                    .interrupt_active_cells()
                    .await;
            }
            ControlFlow::Continue(false) => {}
        }
    }
}

fn matching_turn<'a>(
    active: &'a mut Option<ActiveTurn>,
    turn: &TurnContext,
) -> Option<&'a mut ActiveTurn> {
    active.as_mut().filter(|active| {
        active
            .task
            .as_ref()
            .is_some_and(|task| task.turn_context.sub_id == turn.sub_id)
    })
}
