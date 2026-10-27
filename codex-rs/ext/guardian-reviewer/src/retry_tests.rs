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
fn guardian_review_retry_only_retries_recoverable_errors() {
    let assessment = GuardianAssessment {
        risk_level: GuardianRiskLevel::High,
        user_authorization: GuardianUserAuthorization::Unknown,
        outcome: GuardianAssessmentOutcome::Deny,
        rationale: "deny".to_string(),
    };
    let transient_error_info = [
        CodexErrorInfo::ServerOverloaded,
        CodexErrorInfo::PolicyCapacityUnavailable,
        CodexErrorInfo::FlexUnavailable,
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
            GuardianReviewOutcome::Error(GuardianReviewError::StaleAuthorization),
            true,
        ),
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
            GuardianReviewOutcome::Error(GuardianReviewError::session_with_error_info(
                anyhow::anyhow!("bio policy"),
                CodexErrorInfo::BioPolicy,
            )),
            false,
        ),
        (
            GuardianReviewOutcome::Error(GuardianReviewError::session_with_error_info(
                anyhow::anyhow!("invalid prompt"),
                CodexErrorInfo::InvalidPrompt,
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

#[test_case::test_case(Some(Duration::from_secs(1)); "finite")]
#[test_case::test_case(None; "unlimited")]
#[tokio::test]
async fn guardian_review_retry_wait_honors_cancellation(timeout: Option<Duration>) {
    let cancel_token = CancellationToken::new();
    cancel_token.cancel();

    let error = wait_before_guardian_retry(
        /*attempt_count*/ 1,
        /*max_attempts*/ 3,
        &GuardianReviewOutcome::Error(GuardianReviewError::StaleAuthorization),
        /*retry_not_before*/ None,
        ReviewDeadline::from_timeout(timeout),
        Some(&cancel_token),
    )
    .await;

    assert!(matches!(error, Some(GuardianReviewError::Cancelled)));
}

#[tokio::test]
async fn guardian_review_retry_wait_honors_deadline() {
    let error = wait_before_guardian_retry(
        /*attempt_count*/ 1,
        /*max_attempts*/ 3,
        &GuardianReviewOutcome::Error(GuardianReviewError::StaleAuthorization),
        /*retry_not_before*/ None,
        ReviewDeadline::At(Instant::now()),
        /*external_cancel*/ None,
    )
    .await;

    assert!(matches!(error, Some(GuardianReviewError::Timeout)));
}

#[test_case::test_case(2, Some(Duration::from_secs(90)); "recovers with fresh evidence")]
#[test_case::test_case(4, Some(Duration::from_secs(90)); "exhausts the shared budget")]
#[test_case::test_case(4, None; "unlimited exhausts attempt budget")]
#[tokio::test(start_paused = true)]
async fn stale_authorization_uses_the_shared_attempt_budget(
    success_on_attempt: i64,
    timeout: Option<Duration>,
) {
    let mut attempts = 0;
    let started_at = Instant::now();
    let deadline = ReviewDeadline::from_timeout(timeout);
    let (outcome, analytics, evidence) = run_with_retry(
        GuardianReviewSessionLimits {
            max_attempts: 3,
            deadline,
        },
        /*external_cancel*/ None,
        /*telemetry*/ None,
        |attempt_deadline| {
            assert_eq!(attempt_deadline, deadline);
            attempts += 1;
            let attempt = attempts;
            async move {
                (
                    if attempt == success_on_attempt {
                        GuardianReviewOutcome::CachedApproval
                    } else {
                        GuardianReviewOutcome::Error(GuardianReviewError::StaleAuthorization)
                    },
                    GuardianReviewAnalyticsResult::without_session(),
                    Some(attempt),
                )
            }
        },
    )
    .await;
    assert_eq!(
        started_at.elapsed(),
        Duration::ZERO,
        "stale authorization retries should not wait for service backoff"
    );
    if success_on_attempt <= 3 {
        assert!(matches!(outcome, GuardianReviewOutcome::CachedApproval));
    } else {
        assert!(matches!(
            outcome,
            GuardianReviewOutcome::Error(GuardianReviewError::StaleAuthorization)
        ));
    }
    let expected_attempts = success_on_attempt.min(3);
    assert_eq!(
        (attempts, analytics.attempt_count, evidence),
        (
            expected_attempts,
            expected_attempts,
            Some(expected_attempts)
        )
    );
}

#[tokio::test]
async fn stale_authorization_does_not_extend_the_deadline() {
    let deadline = ReviewDeadline::At(Instant::now() + Duration::from_secs(/*secs*/ 1));
    let mut attempts = 0;
    let (outcome, analytics, _) = run_with_retry(
        GuardianReviewSessionLimits {
            max_attempts: 3,
            deadline,
        },
        /*external_cancel*/ None,
        /*telemetry*/ None,
        |attempt_deadline| {
            attempts += 1;
            assert_eq!(attempt_deadline, deadline);
            async move {
                deadline.wait().await;
                (
                    GuardianReviewOutcome::Error(GuardianReviewError::StaleAuthorization),
                    GuardianReviewAnalyticsResult::without_session(),
                    None::<()>,
                )
            }
        },
    )
    .await;
    assert!(matches!(
        outcome,
        GuardianReviewOutcome::Error(GuardianReviewError::Timeout)
    ));
    assert_eq!((attempts, analytics.attempt_count), (1, 1));
}
