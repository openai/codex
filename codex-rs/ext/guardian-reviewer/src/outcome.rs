//! Retains parsed reviewer responses through feedback capture and distinguishes
//! completed assessments from failures without assigning risk to errors.

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
    Completed(anyhow::Result<Option<GuardianReviewResponse>>),
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

/// A completed response parsed once, with its original text retained for feedback.
/// Invalid JSON remains a completed response so history and session reuse are unchanged.
pub struct GuardianReviewResponse {
    raw: String,
    assessment: anyhow::Result<GuardianAssessment>,
}

impl GuardianReviewResponse {
    pub fn new(raw: String) -> Self {
        let assessment = crate::parse_guardian_assessment(&raw);
        Self { raw, assessment }
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn assessment(&self) -> Result<&GuardianAssessment, &anyhow::Error> {
        self.assessment.as_ref()
    }
}

impl std::fmt::Debug for GuardianReviewResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GuardianReviewResponse")
            .field("valid", &self.assessment.is_ok())
            .finish_non_exhaustive()
    }
}

impl GuardianReviewSessionOutcome {
    /// Extract the assessment produced by one reviewer session, preserving failures.
    pub fn into_assessment(self) -> Result<GuardianAssessment, GuardianReviewError> {
        match self {
            Self::Completed(Ok(Some(response))) => {
                response.assessment.map_err(GuardianReviewError::parse)
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
