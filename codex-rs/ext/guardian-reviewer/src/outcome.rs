//! Distinguishes completed assessments from failures without assigning risk to errors.

use crate::GuardianAssessment;
use codex_analytics::GuardianReviewFailureReason;
use codex_protocol::protocol::CodexErrorInfo;
use tokio::time::Instant;

#[derive(Debug)]
pub enum GuardianReviewOutcome {
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
}

impl From<GuardianReviewSessionOutcome> for GuardianReviewOutcome {
    fn from(outcome: GuardianReviewSessionOutcome) -> Self {
        match outcome {
            GuardianReviewSessionOutcome::Completed(Ok(Some(message))) => {
                match crate::parse_guardian_assessment(Some(&message)) {
                    Ok(assessment) => Self::Completed(assessment),
                    Err(error) => Self::Error(GuardianReviewError::parse(error)),
                }
            }
            GuardianReviewSessionOutcome::Completed(Ok(None)) => {
                Self::Error(GuardianReviewError::session(anyhow::anyhow!(
                    "guardian review completed without an assessment payload"
                )))
            }
            GuardianReviewSessionOutcome::Completed(Err(error)) => {
                Self::Error(GuardianReviewError::session(error))
            }
            GuardianReviewSessionOutcome::PromptBuildFailed(error) => {
                Self::Error(GuardianReviewError::prompt_build(error))
            }
            GuardianReviewSessionOutcome::InputBudgetExceeded => {
                Self::Error(GuardianReviewError::InputBudgetExceeded)
            }
            GuardianReviewSessionOutcome::SessionFailed {
                error,
                error_info,
                retry_at,
            } => Self::Error(GuardianReviewError::Session {
                message: error.to_string(),
                error_info,
                retry_at,
            }),
            GuardianReviewSessionOutcome::TimedOut => Self::Error(GuardianReviewError::Timeout),
            GuardianReviewSessionOutcome::Aborted => Self::Error(GuardianReviewError::Cancelled),
        }
    }
}
