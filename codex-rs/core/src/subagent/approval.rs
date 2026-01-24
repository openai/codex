//! Approval routing for subagent tool execution.

use super::definition::ApprovalMode;

/// Router for handling approval requests from subagents.
#[derive(Debug, Clone)]
pub struct ApprovalRouter {
    /// The approval mode for this subagent.
    mode: ApprovalMode,
}

impl ApprovalRouter {
    /// Create a new approval router.
    pub fn new(mode: ApprovalMode) -> Self {
        Self { mode }
    }

    /// Check if approvals should be auto-approved.
    pub fn is_auto_approve(&self) -> bool {
        matches!(self.mode, ApprovalMode::AutoApprove | ApprovalMode::DontAsk)
    }

    /// Check if approvals should be routed to parent.
    pub fn should_route_to_parent(&self) -> bool {
        matches!(self.mode, ApprovalMode::RouteToParent)
    }

    /// Get the approval mode.
    pub fn mode(&self) -> ApprovalMode {
        self.mode
    }
}

impl Default for ApprovalRouter {
    fn default() -> Self {
        Self::new(ApprovalMode::RouteToParent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_approve() {
        let router = ApprovalRouter::new(ApprovalMode::AutoApprove);
        assert!(router.is_auto_approve());
        assert!(!router.should_route_to_parent());
    }

    #[test]
    fn test_route_to_parent() {
        let router = ApprovalRouter::new(ApprovalMode::RouteToParent);
        assert!(!router.is_auto_approve());
        assert!(router.should_route_to_parent());
    }

    #[test]
    fn test_dont_ask() {
        let router = ApprovalRouter::new(ApprovalMode::DontAsk);
        assert!(router.is_auto_approve());
        assert!(!router.should_route_to_parent());
    }

    #[test]
    fn test_default() {
        let router = ApprovalRouter::default();
        assert!(router.should_route_to_parent());
    }
}
