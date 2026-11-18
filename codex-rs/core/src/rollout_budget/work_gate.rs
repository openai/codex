//! Admission for new rollout work and checkpoints within a turn.

use super::ThreadBudgetPolicy;
use crate::client_common::SamplingCharge;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_protocol::protocol::SessionSource;
use std::ops::ControlFlow;
use std::sync::OnceLock;

#[derive(Debug, Default)]
pub struct RolloutWorkGate(OnceLock<()>);

impl RolloutWorkGate {
    pub(super) fn stop_budget(&self) {
        let _ = self.0.set(());
    }

    pub(super) fn is_stopped(&self) -> bool {
        self.0.get().is_some()
    }

    pub(super) fn check_start(&self) -> Result<()> {
        if self.is_stopped() {
            return Err(CodexErr::SessionBudgetExceeded);
        }
        Ok(())
    }

    pub(super) fn can_receive_turn_input(&self, _thread: ThreadId, _turn: &str) -> bool {
        !self.is_stopped()
    }
}

pub(super) fn defer_compaction_start(
    _policy: &ThreadBudgetPolicy,
    _source: &SessionSource,
    _phase: codex_analytics::CompactionPhase,
) -> bool {
    false
}

pub(crate) struct WorkCheckpoint;

impl WorkCheckpoint {
    pub(crate) fn should_retry(&self, _needs_follow_up: bool) -> bool {
        false
    }

    pub(crate) fn is_finished(&self) -> bool {
        false
    }

    pub(crate) fn should_return(&self, _outcome: &codex_hooks::StopOutcome) -> bool {
        false
    }
}

pub(crate) struct TurnWork {
    policy: ThreadBudgetPolicy,
}

impl TurnWork {
    pub(crate) fn new(
        policy: &ThreadBudgetPolicy,
        _thread: ThreadId,
        _turn: &str,
        _source: &SessionSource,
    ) -> Self {
        Self {
            policy: policy.clone(),
        }
    }

    pub(crate) fn accept_response(&mut self, _has_final_answer: bool) -> Result<WorkCheckpoint> {
        self.policy.check_start()?;
        Ok(WorkCheckpoint)
    }

    pub(crate) fn assistant_message(
        &mut self,
        final_answer: Option<String>,
        commentary: Option<String>,
    ) -> Option<String> {
        commentary.or(final_answer)
    }

    pub(crate) fn after_stop_hook(&self) -> Result<WorkCheckpoint> {
        Ok(WorkCheckpoint)
    }

    pub(crate) fn should_finish_after_compaction(&self) -> Result<bool> {
        Ok(false)
    }

    pub(crate) fn on_error(
        &self,
        error: CodexErr,
    ) -> Result<ControlFlow<WorkCheckpoint, CodexErr>> {
        Ok(ControlFlow::Continue(error))
    }

    pub(crate) fn take_answer(&mut self) -> Option<String> {
        None
    }
}

#[derive(Default)]
pub(crate) struct ResponseWork {}

impl ResponseWork {
    pub(crate) fn accept_charge(&mut self, _charge: &mut SamplingCharge) {}

    pub(crate) fn finish(self, _policy: &ThreadBudgetPolicy) -> Result<()> {
        Ok(())
    }
}
