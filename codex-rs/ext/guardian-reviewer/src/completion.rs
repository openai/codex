//! Converts reviewer outcomes into decisions and reports. The host publishes these
//! reports and applies them only to the action whose evidence was reviewed.

use codex_analytics::GuardianReviewAnalyticsResult;
use codex_analytics::GuardianReviewDecision;
use codex_analytics::GuardianReviewTerminalStatus;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::GuardianAssessmentOutcome;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::GuardianRiskLevel;
use codex_protocol::protocol::GuardianUserAuthorization;
use codex_protocol::protocol::ReviewDecision;

use crate::GuardianReviewError;
use crate::GuardianReviewOutcome;

const REJECTION_INSTRUCTIONS: &str = concat!(
    "The agent must not attempt to achieve the same outcome via workaround, ",
    "indirect execution, or policy circumvention. ",
    "Proceed only with a materially safer alternative, ",
    "or if the user explicitly approves the action after being informed of the risk. ",
    "Otherwise, stop and request user input.",
);
const REVIEW_FAILURE_INSTRUCTIONS: &str = concat!(
    "The action was not executed because automatic approval review could not be completed. ",
    "This is a review failure, not a determination that the action is unsafe. ",
    "Do not bypass the approval check; resolve the error or ask the user for guidance.",
);
const TIMEOUT_INSTRUCTIONS: &str = concat!(
    "The automatic permission approval review did not finish before its deadline. ",
    "Do not assume the action is unsafe based on the timeout alone. ",
    "You may retry once, or ask the user for guidance or explicit approval.",
);
const INPUT_BUDGET_MESSAGE: &str =
    "the complete action and minimum review context exceed the reviewer input budget";

pub fn guardian_timeout_message(model_info: &ModelInfo) -> String {
    model_info
        .model_messages
        .as_ref()
        .and_then(|messages| messages.auto_review.as_ref())
        .and_then(|messages| messages.timeout_instructions.as_deref())
        .unwrap_or(TIMEOUT_INSTRUCTIONS)
        .to_string()
}

pub struct ReviewCompletion {
    /// `None` requests user approval after optional review exhausts its input budget.
    pub decision: Option<ReviewDecision>,
    pub event: GuardianAssessmentEvent,
    pub warning: Option<String>,
    pub analytics: GuardianReviewAnalyticsResult,
    /// Only completed assessments may enter the evidence cache or count as policy denials.
    pub assessment_outcome: Option<GuardianAssessmentOutcome>,
}

pub fn complete_review(
    outcome: GuardianReviewOutcome,
    model: &ModelInfo,
    require_guardian: bool,
    mut event: GuardianAssessmentEvent,
    mut analytics: GuardianReviewAnalyticsResult,
) -> ReviewCompletion {
    let completed_assessment = match &outcome {
        GuardianReviewOutcome::Completed(assessment) => Some(assessment.outcome),
        GuardianReviewOutcome::Error(_) => None,
    };
    let assessment = match outcome {
        GuardianReviewOutcome::Completed(assessment) => {
            let approved = matches!(assessment.outcome, GuardianAssessmentOutcome::Allow);
            analytics.decision = if approved {
                GuardianReviewDecision::Approved
            } else {
                GuardianReviewDecision::Denied
            };
            analytics.terminal_status = if approved {
                GuardianReviewTerminalStatus::Approved
            } else {
                GuardianReviewTerminalStatus::Denied
            };
            analytics.failure_reason = None;
            analytics.risk_level = Some(assessment.risk_level);
            analytics.user_authorization = Some(assessment.user_authorization);
            analytics.outcome = Some(assessment.outcome);
            assessment
        }
        GuardianReviewOutcome::Error(error) => {
            analytics.failure_reason = Some(error.failure_reason());
            if matches!(error, GuardianReviewError::InputBudgetExceeded) && !require_guardian {
                let rationale = format!("Automatic approval review failed: {INPUT_BUDGET_MESSAGE}");
                analytics.decision = GuardianReviewDecision::Aborted;
                analytics.terminal_status = GuardianReviewTerminalStatus::Aborted;
                event.status = GuardianAssessmentStatus::Aborted;
                event.rationale = Some(rationale.clone());
                return ReviewCompletion {
                    decision: None,
                    event,
                    warning: Some(rationale),
                    analytics,
                    assessment_outcome: None,
                };
            }
            match error {
                GuardianReviewError::Timeout => {
                    let rationale = "Automatic approval review timed out while evaluating the requested approval.".to_string();
                    analytics.decision = GuardianReviewDecision::Denied;
                    analytics.terminal_status = GuardianReviewTerminalStatus::TimedOut;
                    event.status = GuardianAssessmentStatus::TimedOut;
                    event.rationale = Some(rationale.clone());
                    return ReviewCompletion {
                        decision: Some(ReviewDecision::TimedOut),
                        event,
                        warning: Some(rationale),
                        analytics,
                        assessment_outcome: None,
                    };
                }
                GuardianReviewError::Cancelled => {
                    analytics.decision = GuardianReviewDecision::Aborted;
                    analytics.terminal_status = GuardianReviewTerminalStatus::Aborted;
                    event.status = GuardianAssessmentStatus::Aborted;
                    return ReviewCompletion {
                        decision: Some(ReviewDecision::Abort),
                        event,
                        warning: None,
                        analytics,
                        assessment_outcome: None,
                    };
                }
                GuardianReviewError::InputBudgetExceeded
                | GuardianReviewError::PromptBuild { .. }
                | GuardianReviewError::Session { .. }
                | GuardianReviewError::Parse { .. } => {
                    let message = match &error {
                        GuardianReviewError::InputBudgetExceeded => INPUT_BUDGET_MESSAGE,
                        GuardianReviewError::PromptBuild { message }
                        | GuardianReviewError::Session { message, .. }
                        | GuardianReviewError::Parse { message } => message,
                        GuardianReviewError::Timeout | GuardianReviewError::Cancelled => {
                            "guardian review failed"
                        }
                    };
                    analytics.decision = GuardianReviewDecision::Denied;
                    analytics.terminal_status = GuardianReviewTerminalStatus::FailedClosed;
                    let rationale = format!("Automatic approval review failed: {message}");
                    // Keep the existing blocked status for client compatibility.
                    event.status = GuardianAssessmentStatus::Denied;
                    event.rationale = Some(rationale.clone());
                    return ReviewCompletion {
                        decision: Some(ReviewDecision::denied(format!(
                            "{rationale}\n{REVIEW_FAILURE_INSTRUCTIONS}"
                        ))),
                        event,
                        warning: Some(rationale),
                        analytics,
                        assessment_outcome: None,
                    };
                }
            }
        }
    };
    let approved = matches!(assessment.outcome, GuardianAssessmentOutcome::Allow);
    let verdict = if approved { "approved" } else { "denied" };
    let authorization = match assessment.user_authorization {
        GuardianUserAuthorization::Unknown => "unknown",
        GuardianUserAuthorization::Low => "low",
        GuardianUserAuthorization::Medium => "medium",
        GuardianUserAuthorization::High => "high",
    };
    let risk = match assessment.risk_level {
        GuardianRiskLevel::Low => "low",
        GuardianRiskLevel::Medium => "medium",
        GuardianRiskLevel::High => "high",
        GuardianRiskLevel::Critical => "critical",
    };
    let warning = format!(
        "Automatic approval review {verdict} (risk: {risk}, authorization: {authorization}): {}",
        assessment.rationale
    );
    event.status = if approved {
        GuardianAssessmentStatus::Approved
    } else {
        GuardianAssessmentStatus::Denied
    };
    event.risk_level = Some(assessment.risk_level);
    event.user_authorization = Some(assessment.user_authorization);
    event.rationale = Some(assessment.rationale.clone());
    let decision = if approved {
        ReviewDecision::Approved
    } else {
        let rationale = if assessment.rationale.trim().is_empty() {
            "Auto-reviewer denied the action without a specific rationale."
        } else {
            assessment.rationale.trim()
        };
        let instructions = model
            .model_messages
            .as_ref()
            .and_then(|messages| messages.auto_review.as_ref())
            .and_then(|messages| messages.rejection_instructions.as_deref())
            .unwrap_or(REJECTION_INSTRUCTIONS);
        ReviewDecision::denied(format!(
            "This action was rejected due to unacceptable risk.\nReason: {rationale}\n{instructions}"
        ))
    };
    ReviewCompletion {
        decision: Some(decision),
        event,
        warning: Some(warning),
        analytics,
        assessment_outcome: completed_assessment,
    }
}
