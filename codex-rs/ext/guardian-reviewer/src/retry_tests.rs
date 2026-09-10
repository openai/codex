#[cfg(test)]
use super::*;
use crate::GuardianAssessment;
use codex_analytics::GuardianReviewFailureReason;
use codex_protocol::protocol::GuardianAssessmentOutcome;
use codex_protocol::protocol::GuardianRiskLevel;
use codex_protocol::protocol::GuardianUserAuthorization;
use pretty_assertions::assert_eq;
use std::time::Duration;

#[test]
fn guardian_review_error_reason_distinguishes_error_kinds() {
    let parse_error = GuardianReviewError::parse(anyhow::anyhow!("bad guardian JSON"));
    let prompt_error = GuardianReviewError::prompt_build(anyhow::anyhow!("bad prompt/config"));
    let session_error = GuardianReviewError::session(anyhow::anyhow!("guardian runtime failed"));
    let structured_session_error = GuardianReviewError::session_with_error_info(
        anyhow::anyhow!("temporary guardian failure"),
        CodexErrorInfo::ServerOverloaded,
    );

    assert!(matches!(
        parse_error.failure_reason(),
        GuardianReviewFailureReason::ParseError
    ));
    assert!(matches!(
        prompt_error.failure_reason(),
        GuardianReviewFailureReason::PromptBuildError
    ));
    assert!(matches!(
        session_error.failure_reason(),
        GuardianReviewFailureReason::SessionError
    ));
    assert!(matches!(
        structured_session_error.failure_reason(),
        GuardianReviewFailureReason::SessionError
    ));
}

#[test]
fn guardian_review_retry_only_retries_transient_session_and_parse_errors() {
    let assessment = GuardianAssessment {
        risk_level: GuardianRiskLevel::High,
        user_authorization: GuardianUserAuthorization::Unknown,
        outcome: GuardianAssessmentOutcome::Deny,
        rationale: "deny".to_string(),
    };
    let transient_error_info = [
        CodexErrorInfo::ServerOverloaded,
        CodexErrorInfo::HttpConnectionFailed {
            http_status_code: Some(502),
        },
        CodexErrorInfo::ResponseStreamConnectionFailed {
            http_status_code: Some(503),
        },
        CodexErrorInfo::InternalServerError,
        CodexErrorInfo::ResponseStreamDisconnected {
            http_status_code: None,
        },
    ];
    let mut outcomes = transient_error_info
        .into_iter()
        .map(|error_info| {
            (
                GuardianReviewOutcome::Error(GuardianReviewError::session_with_error_info(
                    anyhow::anyhow!("transient session"),
                    error_info,
                )),
                true,
            )
        })
        .collect::<Vec<_>>();
    outcomes.extend([
        (GuardianReviewOutcome::Completed(assessment), false),
        (
            GuardianReviewOutcome::Error(GuardianReviewError::InputBudgetExceeded),
            false,
        ),
        (
            GuardianReviewOutcome::Error(GuardianReviewError::prompt_build(anyhow::anyhow!(
                "prompt"
            ))),
            false,
        ),
        (
            GuardianReviewOutcome::Error(GuardianReviewError::session(anyhow::anyhow!("session"))),
            false,
        ),
        (
            GuardianReviewOutcome::Error(GuardianReviewError::session_with_error_info(
                anyhow::anyhow!("bad request"),
                CodexErrorInfo::BadRequest,
            )),
            false,
        ),
        (
            GuardianReviewOutcome::Error(GuardianReviewError::parse(anyhow::anyhow!("parse"))),
            true,
        ),
        (
            GuardianReviewOutcome::Error(GuardianReviewError::Timeout),
            false,
        ),
        (
            GuardianReviewOutcome::Error(GuardianReviewError::Cancelled),
            false,
        ),
    ]);

    for (outcome, expected) in outcomes {
        assert_eq!(should_retry_guardian_review(&outcome), expected);
    }
}

#[tokio::test]
async fn guardian_review_retry_wait_honors_cancellation() {
    let cancel_token = CancellationToken::new();
    cancel_token.cancel();

    let error = wait_before_guardian_retry(
        /*attempt_count*/ 1,
        /*retry_not_before*/ None,
        Instant::now() + Duration::from_secs(/*secs*/ 1),
        Some(&cancel_token),
    )
    .await;

    assert!(matches!(error, Some(GuardianReviewError::Cancelled)));
}

#[tokio::test]
async fn guardian_review_retry_wait_honors_deadline() {
    let error = wait_before_guardian_retry(
        /*attempt_count*/ 1,
        /*retry_not_before*/ None,
        Instant::now(),
        /*external_cancel*/ None,
    )
    .await;

    assert!(matches!(error, Some(GuardianReviewError::Timeout)));
}
