//! Agent handler stub.
//!
//! Delegates execution to a sub-agent. Currently a stub that returns
//! `Continue`.

use tracing::debug;

use crate::result::HookResult;

/// Handles hooks that delegate to a sub-agent.
pub struct AgentHandler;

impl AgentHandler {
    /// Stub implementation. Returns `Continue` for now.
    ///
    /// In the future this will spawn a sub-agent with up to `max_turns` turns.
    pub fn execute(max_turns: i32) -> HookResult {
        debug!(max_turns, "Agent hook stub invoked (not yet implemented)");
        HookResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_returns_continue() {
        let result = AgentHandler::execute(5);
        assert!(matches!(result, HookResult::Continue));
    }

    #[test]
    fn test_stub_with_different_turns() {
        let result = AgentHandler::execute(10);
        assert!(matches!(result, HookResult::Continue));
    }
}
