//! Host adapter types and assessment results for synchronous Guardian review.
//! The extension supplies agent startup and owns the reviewer pool and its lifecycle.

pub use crate::guardian::GuardianReviewSession;
pub use crate::guardian::GuardianReviewState;
pub use crate::guardian::PreparedGuardianContext;
pub use crate::guardian::prepare_review_prewarm;
pub use crate::mcp_tool_call::conversation_history::conversation_history_tools;

/// The result of checking a recorded call without executing it.
#[derive(Debug, PartialEq, Eq)]
pub struct GuardianReviewResult {
    pub call_id: String,
    /// Absent when exec policy skips approval.
    pub assessment: Option<codex_guardian_reviewer::GuardianAssessment>,
}
