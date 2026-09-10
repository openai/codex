//! Applies one review's attempt budget, deadline and cancellation to host-owned sessions.
//! Resubmissions are independent; this layer keeps no state between approval requests.

use crate::GuardianReviewError;
use crate::GuardianReviewOutcome;
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
    pub deadline: Instant,
}

/// Retries only recoverable review failures within one shared deadline.
pub async fn run_with_retry<F, Attempt>(
    limits: GuardianReviewSessionLimits,
    external_cancel: Option<&CancellationToken>,
    mut run_attempt: F,
) -> (GuardianReviewOutcome, GuardianReviewAnalyticsResult)
where
    F: FnMut(Instant) -> Attempt,
    Attempt: Future<Output = (GuardianReviewOutcome, GuardianReviewAnalyticsResult)>,
{
    let GuardianReviewSessionLimits {
        max_attempts,
        deadline,
    } = limits;
    assert!(max_attempts > 0, "guardian review must run at least once");
    let mut attempt_count = 1;
    loop {
        let (outcome, mut analytics_result) = run_attempt(deadline).await;
        analytics_result.attempt_count = attempt_count;
        if attempt_count >= max_attempts || !should_retry_guardian_review(&outcome) {
            return (outcome, analytics_result);
        }
        let retry_at = match &outcome {
            GuardianReviewOutcome::Error(GuardianReviewError::Session { retry_at, .. }) => {
                *retry_at
            }
            _ => None,
        };
        if let Some(error) =
            wait_before_guardian_retry(attempt_count, retry_at, deadline, external_cancel).await
        {
            return (GuardianReviewOutcome::Error(error), analytics_result);
        }
        attempt_count += 1;
    }
}

async fn wait_before_guardian_retry(
    attempt_count: i64,
    retry_not_before: Option<Instant>,
    deadline: Instant,
    external_cancel: Option<&CancellationToken>,
) -> Option<GuardianReviewError> {
    let exponential_delay = 200.0 * 2.0_f64.powi(attempt_count.saturating_sub(1) as i32);
    let jitter = rand::rng().random_range(0.9..1.1);
    let retry_delay = Duration::from_millis((exponential_delay * jitter) as u64);
    let retry_at = (Instant::now() + retry_delay)
        .max(retry_not_before.unwrap_or_else(Instant::now))
        .min(deadline);
    tokio::select! {
        _ = sleep_until(retry_at) => {
            (Instant::now() >= deadline).then_some(GuardianReviewError::Timeout)
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
        GuardianReviewOutcome::Error(GuardianReviewError::Parse { .. }) => true,
        GuardianReviewOutcome::Error(GuardianReviewError::Session {
            error_info: Some(error),
            ..
        }) => match error {
            CodexErrorInfo::RateLimitExceeded
            | CodexErrorInfo::ServerOverloaded
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
            | CodexErrorInfo::MisalignmentPolicyViolation
            | CodexErrorInfo::Unauthorized
            | CodexErrorInfo::BadRequest
            | CodexErrorInfo::SandboxError
            | CodexErrorInfo::ActiveTurnNotSteerable { .. }
            | CodexErrorInfo::ThreadRollbackFailed
            | CodexErrorInfo::Other => false,
        },
        GuardianReviewOutcome::Completed(_)
        | GuardianReviewOutcome::Error(
            GuardianReviewError::PromptBuild { .. }
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
