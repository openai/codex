use std::error::Error;
use std::fmt;

use codex_protocol::approvals::GuardianAssessmentAction;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;

use crate::ExtensionData;

/// Identifies the runtime that originated an approval-review request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalReviewSource {
    MainTurn,
    DelegatedSubagent,
}

/// Structured input supplied to an approval-review extension.
///
/// The stores let an implementation retain private state at the matching
/// lifetime without exposing host runtime objects.
#[derive(Clone, Copy)]
pub struct ApprovalReviewInput<'a> {
    pub session_store: &'a ExtensionData,
    pub thread_store: &'a ExtensionData,
    pub turn_store: &'a ExtensionData,
    pub review_id: &'a str,
    pub turn_id: &'a str,
    pub target_item_id: Option<&'a str>,
    /// Bounded review prompt rendered by the host.
    ///
    /// The action is useful assessment metadata but does not yet carry all
    /// details needed to reproduce the current review prompt.
    pub prompt: &'a str,
    pub action: &'a GuardianAssessmentAction,
    pub reviewer: ApprovalsReviewer,
    pub approval_policy: &'a AskForApproval,
    pub retry_reason: Option<&'a str>,
    pub source: ApprovalReviewSource,
}

/// Result of offering an approval request to one extension contributor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalReviewOutcome {
    /// The contributor does not own this request; dispatch should continue.
    Abstain,
    /// The contributor claims the request with an authoritative decision.
    Decision {
        decision: ReviewDecision,
        /// Contributor-owned rationale to return when the decision rejects the request.
        denial_message: Option<String>,
    },
}

impl ApprovalReviewOutcome {
    /// Claims the request with a decision that does not need a denial rationale.
    pub fn decision(decision: ReviewDecision) -> Self {
        Self::Decision {
            decision,
            denial_message: None,
        }
    }

    /// Denies the request with a contributor-owned rationale for the caller.
    pub fn denied(message: impl Into<String>) -> Self {
        Self::Decision {
            decision: ReviewDecision::Denied,
            denial_message: Some(message.into()),
        }
    }
}

/// Failure while an extension reviews an approval request.
///
/// The registry stops dispatch on errors so the host can fail closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalReviewError {
    message: String,
}

impl ApprovalReviewError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ApprovalReviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl Error for ApprovalReviewError {}
