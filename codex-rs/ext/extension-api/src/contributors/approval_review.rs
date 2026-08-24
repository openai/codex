use std::fmt;
use std::sync::Arc;

use codex_protocol::ThreadId;
use codex_protocol::approvals::GuardianAssessmentOutcome;
use codex_protocol::approvals::GuardianRiskLevel;
use codex_protocol::approvals::GuardianUserAuthorization;
use serde_json::Value;

use crate::ConversationHistorySnapshot;
use crate::ExtensionData;

/// Classification returned by an approval reviewer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalAssessment {
    /// Final allow-or-deny decision for the requested action.
    pub outcome: GuardianAssessmentOutcome,
    /// Risk level assigned to the action by the reviewer.
    pub risk_level: GuardianRiskLevel,
    /// Whether the conversation authorizes the requested action.
    pub user_authorization: GuardianUserAuthorization,
    /// Human-readable explanation of the assessment.
    pub rationale: String,
}

/// Operational failure returned by an approval reviewer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApprovalReviewError {
    /// The reviewer could not produce a valid assessment.
    Failed(String),
    /// The request exceeded its host-owned review deadline.
    TimedOut,
    /// The request was cancelled by its parent approval.
    Cancelled,
}

impl fmt::Display for ApprovalReviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(message) => write!(formatter, "approval review failed: {message}"),
            Self::TimedOut => formatter.write_str("approval review timed out"),
            Self::Cancelled => formatter.write_str("approval review was cancelled"),
        }
    }
}

impl std::error::Error for ApprovalReviewError {}

/// Immutable context for reviewing an existing host-owned approval action.
pub struct ApprovalReviewInput<'a> {
    /// Structured action evidence rendered by the host's existing approval path.
    pub action: &'a Value,
    /// Canonical immutable conversation snapshot captured before review.
    pub conversation_history: Arc<dyn ConversationHistorySnapshot>,
    /// Stable host-owned thread identifier.
    pub thread_id: ThreadId,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
    /// Stable host-owned turn identifier.
    pub turn_id: &'a str,
    /// Policy reason that caused the host to request approval.
    pub approval_reason: Option<&'a str>,
    /// Reason an earlier request is being retried, when applicable.
    pub retry_reason: Option<&'a str>,
}
