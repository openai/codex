//! Applies one review's attempt budget, deadline and cancellation to host-owned sessions.
//! Resubmissions are independent; this layer keeps no state between approval requests.
//! Changed local evidence retries cooperatively without service backoff, under the same limits.

use crate::GuardianReviewError;
use crate::GuardianReviewOutcome;
use crate::ReviewDeadline;
use codex_analytics::GuardianReviewAnalyticsResult;
use codex_protocol::protocol::CodexErrorInfo;
use rand::Rng;
use std::future::Future;
use std::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep_until;
use tokio_util::sync::CancellationToken;

pub struct GuardianReviewSessionLimits {
    pub max_attempts: i64,
    pub deadline: ReviewDeadline,
}

/// Retries only recoverable review failures within one shared deadline.
/// Returns the final attempt's evidence without carrying it into subsequent attempts.
pub async fn run_with_retry<Evidence, F, Attempt>(
    limits: GuardianReviewSessionLimits,
    external_cancel: Option<&CancellationToken>,
    telemetry: Option<&codex_otel::SessionTelemetry>,
    mut run_attempt: F,
) -> (
    GuardianReviewOutcome,
    GuardianReviewAnalyticsResult,
    Option<Evidence>,
)
where
    F: FnMut(ReviewDeadline) -> Attempt,
    Attempt: Future<
        Output = (
            GuardianReviewOutcome,
            GuardianReviewAnalyticsResult,
            Option<Evidence>,
        ),
    >,
{
    let GuardianReviewSessionLimits {
        max_attempts,
        deadline,
    } = limits;
    assert!(max_attempts > 0, "guardian review must run at least once");
    let mut attempt_count = 1;
    let mut context_changed = false;
    loop {
        let (outcome, mut analytics_result, evidence) = run_attempt(deadline).await;
        analytics_result.attempt_count = attempt_count;
        context_changed |= analytics_result.context_changed;
        analytics_result.context_changed = context_changed;
        if attempt_count >= max_attempts || !should_retry_guardian_review(&outcome) {
            return (outcome, analytics_result, evidence);
        }
        let retry_at = match &outcome {
            GuardianReviewOutcome::Error(GuardianReviewError::Session { retry_at, .. }) => {
                *retry_at
            }
            _ => None,
        };
        let wait_started = Instant::now();
        let wait_error = wait_before_guardian_retry(
            attempt_count,
            max_attempts,
            &outcome,
            retry_at,
            deadline,
            external_cancel,
        )
        .await;
        if let Some(telemetry) = telemetry {
            let reason = match &outcome {
                GuardianReviewOutcome::Error(error) => {
                    crate::metrics::failure_reason_tag(Some(error.failure_reason()))
                }
                GuardianReviewOutcome::CachedApproval | GuardianReviewOutcome::Completed(_) => {
                    "none"
                }
            };
            let wait_outcome = match &wait_error {
                None => "retry",
                Some(GuardianReviewError::Cancelled) => "cancelled",
                Some(_) => "deadline",
            };
            let tags = [("failure_reason", reason), ("outcome", wait_outcome)];
            telemetry.record_duration(
                "codex.guardian.review.retry_wait_ms",
                wait_started.elapsed(),
                &tags,
            );
            if wait_error.is_none() {
                telemetry.counter("codex.guardian.review.retry", /*inc*/ 1, &tags);
            }
        }
        if let Some(error) = wait_error {
            return (GuardianReviewOutcome::Error(error), analytics_result, None);
        }
        attempt_count += 1;
    }
}

async fn wait_before_guardian_retry(
    attempt_count: i64,
    max_attempts: i64,
    outcome: &GuardianReviewOutcome,
    retry_not_before: Option<Instant>,
    deadline: ReviewDeadline,
    external_cancel: Option<&CancellationToken>,
) -> Option<GuardianReviewError> {
    if matches!(
        outcome,
        GuardianReviewOutcome::Error(GuardianReviewError::StaleAuthorization)
    ) {
        // This is a local evidence refresh, not a failed service request. Yield so
        // cancellation and queued context updates can run before capturing it again.
        tokio::task::yield_now().await;
        return if external_cancel.is_some_and(CancellationToken::is_cancelled) {
            Some(GuardianReviewError::Cancelled)
        } else if deadline.is_elapsed() {
            Some(GuardianReviewError::Timeout)
        } else {
            None
        };
    }
    let exponential_delay = 200.0 * 2.0_f64.powi(attempt_count.saturating_sub(1) as i32);
    let jitter = rand::rng().random_range(0.9..1.1);
    let retry_delay = Duration::from_millis((exponential_delay * jitter) as u64);
    let now = Instant::now();
    let requested_retry_at = (now + retry_delay).max(retry_not_before.unwrap_or(now));
    let retry_at = deadline.cap(requested_retry_at);
    let (failure_reason, error_info) = match outcome {
        GuardianReviewOutcome::Error(error) => (
            Some(error.failure_reason()),
            if let GuardianReviewError::Session { error_info, .. } = error {
                error_info.as_ref()
            } else {
                None
            },
        ),
        GuardianReviewOutcome::CachedApproval | GuardianReviewOutcome::Completed(_) => (None, None),
    };
    tracing::debug!(
        attempt_count,
        next_attempt = attempt_count + 1,
        max_attempts,
        ?failure_reason,
        ?error_info,
        wait_ms = retry_at.saturating_duration_since(now).as_millis() as u64,
        backoff_ms = retry_delay.as_millis() as u64,
        retry_not_before_ms = retry_not_before
            .map(|instant| instant.saturating_duration_since(now).as_millis() as u64),
        remaining_budget_ms = match deadline {
            ReviewDeadline::At(at) => Some(at.saturating_duration_since(now).as_millis() as u64),
            ReviewDeadline::Unlimited => None,
        },
        deadline_limited = matches!(deadline, ReviewDeadline::At(at) if requested_retry_at >= at),
        "Guardian review retry scheduled"
    );
    tokio::select! {
        _ = sleep_until(retry_at) => {
            deadline.is_elapsed().then_some(GuardianReviewError::Timeout)
        }
        _ = async {
            if let Some(cancel_token) = external_cancel {
                cancel_token.cancelled().await;
            } else {
                std::future::pending::<()>().await;
            }
        } => Some(GuardianReviewError::Cancelled),
    }
}

fn should_retry_guardian_review(outcome: &GuardianReviewOutcome) -> bool {
    match outcome {
        GuardianReviewOutcome::Error(
            GuardianReviewError::Parse { .. } | GuardianReviewError::StaleAuthorization,
        ) => true,
        GuardianReviewOutcome::Error(GuardianReviewError::Session {
            error_info: Some(error),
            ..
        }) => match error {
            CodexErrorInfo::RateLimitExceeded
            | CodexErrorInfo::FlexUnavailable
            | CodexErrorInfo::ServerOverloaded
            | CodexErrorInfo::PolicyCapacityUnavailable
            | CodexErrorInfo::InternalServerError => true,
            CodexErrorInfo::HttpConnectionFailed { http_status_code }
            | CodexErrorInfo::ResponseStreamConnectionFailed { http_status_code }
            | CodexErrorInfo::ResponseStreamDisconnected { http_status_code }
            | CodexErrorInfo::ResponseTooManyFailedAttempts { http_status_code } => {
                matches!(http_status_code, None | Some(408 | 429 | 500..=599))
            }
            CodexErrorInfo::ContextWindowExceeded
            | CodexErrorInfo::SessionBudgetExceeded
            | CodexErrorInfo::UsageLimitExceeded
            | CodexErrorInfo::CyberPolicy
            | CodexErrorInfo::BioPolicy
            | CodexErrorInfo::MisalignmentPolicyViolation
            | CodexErrorInfo::TooManyDenials
            | CodexErrorInfo::Unauthorized
            | CodexErrorInfo::BadRequest
            | CodexErrorInfo::ModelNotFound
            | CodexErrorInfo::InvalidPrompt
            | CodexErrorInfo::SandboxError
            | CodexErrorInfo::ActiveTurnNotSteerable { .. }
            | CodexErrorInfo::ThreadRollbackFailed
            | CodexErrorInfo::Other => false,
        },
        GuardianReviewOutcome::CachedApproval
        | GuardianReviewOutcome::Completed(_)
        | GuardianReviewOutcome::Error(
            GuardianReviewError::InputBudgetExceeded
            | GuardianReviewError::PromptBuild { .. }
            | GuardianReviewError::Session {
                error_info: None, ..
            }
            | GuardianReviewError::Timeout
            | GuardianReviewError::Cancelled,
        ) => false,
    }
}

#[cfg(test)]
#[path = "retry_tests.rs"]
mod tests;
