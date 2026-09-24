//! Request-scoped approval decisions. A review only satisfies the review gate; the host enforces permissions.

use std::sync::Arc;

use codex_protocol::ThreadId;

use crate::ExtensionData;

/// Thread-local state installed only after Guardian V2's async classifier initializes.
pub struct GuardianV2Enabled;

/// Guardian's choice for one approval. Synchronous results pass through unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum ApprovalDecision {
    /// Existing async evidence allows this action without synchronous review.
    Allow,
    Reviewed(codex_protocol::protocol::ReviewDecision),
    AskUser,
}

/// Runs an extension-owned synchronous review already bound to an action by the host.
/// Implementations must not reuse an async score. `None` requests the host user
/// flow when automatic review exhausts its input budget and host policy permits it.
pub trait SynchronousApprovalReviewer: Send + Sync {
    fn review(
        &self,
        reason: codex_protocol::approvals::GuardianReviewReason,
    ) -> crate::ExtensionFuture<'_, Option<codex_protocol::protocol::ReviewDecision>>;
}

/// Inputs to Guardian's policy choice. Conversation and scores stay thread-owned.
pub struct ApprovalDecisionInput<'a> {
    pub approval_id: &'a str,
    /// Host tool invocation being approved, absent for approvals without a tool call.
    pub tool_call_id: Option<&'a str>,
    /// Host-resolved permission evidence; unavailable evidence cannot authorize cached approval.
    pub permissions: Option<&'a codex_guardian_context::PermissionContext>,
    pub action: &'a serde_json::Value,
    pub thread_id: ThreadId,
    pub thread_store: &'a ExtensionData,
    pub category: codex_protocol::openai_models::GuardianScope,
    pub approval_policy: codex_protocol::protocol::AskForApproval,
    pub approvals_reviewer: codex_protocol::config_types::ApprovalsReviewer,
    pub require_guardian: bool,
    /// Existing retry and sensitive-action rules require a synchronous review.
    pub require_fresh_review: bool,
    pub full_access: bool,
    pub metrics: Option<Arc<dyn crate::ExtensionMetrics>>,
    pub synchronous_reviewer: &'a dyn SynchronousApprovalReviewer,
}
