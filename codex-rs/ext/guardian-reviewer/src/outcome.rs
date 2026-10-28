//! Distinguishes completed assessments from failures without assigning risk to errors.

use crate::GuardianAssessment;
use codex_analytics::GuardianReviewFailureReason;
use codex_protocol::protocol::CodexErrorInfo;
use tokio::time::Instant;

#[derive(Debug)]
pub enum GuardianReviewOutcome {
    /// Approved by current async evidence; the redundant attempt is cancelled and drained.
    CachedApproval,
    Completed(GuardianAssessment),
    Error(GuardianReviewError),
}

#[derive(Debug)]
pub enum GuardianReviewError {
    InputBudgetExceeded,
    PromptBuild {
        message: String,
    },
    Session {
        message: String,
        error_info: Option<CodexErrorInfo>,
        retry_at: Option<Instant>,
    },
    Parse {
        message: String,
    },
    Timeout,
    Cancelled,
    /// The action is still pending, but its authorization evidence changed during review.
    StaleAuthorization,
}

impl GuardianReviewError {
    pub fn prompt_build(err: anyhow::Error) -> Self {
        Self::PromptBuild {
            message: err.to_string(),
        }
    }

    pub fn session(err: anyhow::Error) -> Self {
        Self::Session {
            message: err.to_string(),
            error_info: None,
            retry_at: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn session_with_error_info(err: anyhow::Error, error_info: CodexErrorInfo) -> Self {
        Self::Session {
            message: err.to_string(),
            error_info: Some(error_info),
            retry_at: None,
        }
    }

    pub fn parse(err: anyhow::Error) -> Self {
        Self::Parse {
            message: err.to_string(),
        }
    }

    pub fn failure_reason(&self) -> GuardianReviewFailureReason {
        match self {
            Self::PromptBuild { .. } => GuardianReviewFailureReason::PromptBuildError,
            Self::Session { .. } | Self::InputBudgetExceeded => {
                GuardianReviewFailureReason::SessionError
            }
            Self::Parse { .. } => GuardianReviewFailureReason::ParseError,
            Self::Timeout => GuardianReviewFailureReason::Timeout,
            Self::Cancelled => GuardianReviewFailureReason::Cancelled,
            Self::StaleAuthorization => GuardianReviewFailureReason::StaleAuthorization,
        }
    }
}

#[derive(Debug)]
pub enum GuardianReviewSessionOutcome {
    Completed(anyhow::Result<Option<String>>),
    PromptBuildFailed(anyhow::Error),
    InputBudgetExceeded,
    SessionFailed {
        error: anyhow::Error,
        error_info: Option<CodexErrorInfo>,
        retry_at: Option<Instant>,
    },
    TimedOut,
    Aborted,
    StaleAuthorization,
}

impl GuardianReviewSessionOutcome {
    /// Extract the assessment produced by one reviewer session, preserving failures.
    pub fn into_assessment(self) -> Result<GuardianAssessment, GuardianReviewError> {
        match self {
            Self::Completed(Ok(Some(message))) => {
                crate::parse_guardian_assessment(&message).map_err(GuardianReviewError::parse)
            }
            Self::Completed(Ok(None)) => Err(GuardianReviewError::session(anyhow::anyhow!(
                "guardian review completed without an assessment payload"
            ))),
            Self::Completed(Err(error)) => Err(GuardianReviewError::session(error)),
            Self::PromptBuildFailed(error) => Err(GuardianReviewError::prompt_build(error)),
            Self::InputBudgetExceeded => Err(GuardianReviewError::InputBudgetExceeded),
            Self::SessionFailed {
                error,
                error_info,
                retry_at,
            } => Err(GuardianReviewError::Session {
                message: error.to_string(),
                error_info,
                retry_at,
            }),
            Self::TimedOut => Err(GuardianReviewError::Timeout),
            Self::Aborted => Err(GuardianReviewError::Cancelled),
            Self::StaleAuthorization => Err(GuardianReviewError::StaleAuthorization),
        }
    }
}

impl From<GuardianReviewSessionOutcome> for GuardianReviewOutcome {
    fn from(outcome: GuardianReviewSessionOutcome) -> Self {
        match outcome.into_assessment() {
            Ok(assessment) => Self::Completed(assessment),
            Err(error) => Self::Error(error),
        }
    }
}
