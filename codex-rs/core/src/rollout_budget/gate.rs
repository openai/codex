//! The first stop closes every work-start boundary in this rollout tree.

use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use std::sync::OnceLock;

/// Controls new work across the rollout tree.
#[derive(Debug, Default)]
pub struct RolloutWorkGate(OnceLock<Stop>);

#[derive(Debug)]
enum Stop {
    Budget,
}

impl RolloutWorkGate {
    pub(super) fn stop_budget(&self) {
        let _ = self.0.set(Stop::Budget);
    }

    pub(super) fn is_stopped(&self) -> bool {
        self.0.get().is_some()
    }

    pub(super) fn check_start(&self) -> Result<()> {
        match self.0.get() {
            None => Ok(()),
            Some(Stop::Budget) => Err(CodexErr::SessionBudgetExceeded),
        }
    }
}
