//! Reconsiders a completed denial once, within the original review's deadline.
//! Only a valid second assessment can replace the denial. Cancellation still aborts;
//! timeouts and other failures keep the denial and discard the session.

use crate::GuardianReviewSessionOutcome;
use crate::ReviewDeadline;
use crate::ReviewTurnResult;
use crate::ReviewerRuntime;
use crate::SessionDisposition;
use crate::start_review_turn;
use crate::wait_for_guardian_review;
use codex_analytics::GuardianReviewAnalyticsResult;
use codex_otel::SessionTelemetry;
use codex_protocol::protocol::GuardianAssessmentOutcome;
use codex_protocol::turn_input::TurnInputRequest;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

/// Runs a review-scoped follow-up without publishing the provisional denial.
/// The caller retains the session lease and original action/authorization snapshot.
pub async fn reconsider_denial(
    runtime: &impl ReviewerRuntime,
    first: ReviewTurnResult,
    request: TurnInputRequest,
    deadline: ReviewDeadline,
    external_cancel: Option<&CancellationToken>,
    analytics: &mut GuardianReviewAnalyticsResult,
    telemetry: &SessionTelemetry,
) -> ReviewTurnResult {
    let GuardianReviewSessionOutcome::Completed(Ok(Some(response))) = &first.outcome else {
        return first;
    };
    if !response
        .assessment()
        .is_ok_and(|assessment| assessment.outcome == GuardianAssessmentOutcome::Deny)
    {
        return first;
    }

    let started = Instant::now();
    let first_token_ms = analytics.time_to_first_token_ms;
    let mut second = match start_review_turn(runtime, request, deadline, external_cancel).await {
        Ok(turn_id) => {
            wait_for_guardian_review(runtime, &turn_id, deadline, external_cancel, analytics).await
        }
        Err(outcome) => ReviewTurnResult {
            outcome,
            disposition: SessionDisposition::Discard,
            turn_completed: false,
        },
    };
    // The enclosing review's latency starts at the first pass. Its caller computes
    // aggregate token usage from the same session after both passes finish.
    analytics.time_to_first_token_ms = first_token_ms;
    let assessment = match &second.outcome {
        GuardianReviewSessionOutcome::Completed(Ok(Some(response))) => response.assessment().ok(),
        GuardianReviewSessionOutcome::Completed(Ok(None) | Err(_))
        | GuardianReviewSessionOutcome::PromptBuildFailed(_)
        | GuardianReviewSessionOutcome::InputBudgetExceeded
        | GuardianReviewSessionOutcome::SessionFailed { .. }
        | GuardianReviewSessionOutcome::StaleAuthorization
        | GuardianReviewSessionOutcome::TimedOut
        | GuardianReviewSessionOutcome::Aborted => None,
    };
    let outcome = match assessment.map(|assessment| assessment.outcome) {
        Some(GuardianAssessmentOutcome::Allow) => "allow",
        Some(GuardianAssessmentOutcome::Deny) => "deny",
        None => match &second.outcome {
            GuardianReviewSessionOutcome::TimedOut => "timed_out",
            GuardianReviewSessionOutcome::Aborted => "cancelled",
            GuardianReviewSessionOutcome::Completed(_)
            | GuardianReviewSessionOutcome::PromptBuildFailed(_)
            | GuardianReviewSessionOutcome::InputBudgetExceeded
            | GuardianReviewSessionOutcome::SessionFailed { .. }
            | GuardianReviewSessionOutcome::StaleAuthorization => "failed",
        },
    };
    let tags = [("outcome", outcome)];
    telemetry.counter(
        "codex.guardian.review.reconsideration",
        /*inc*/ 1,
        &tags,
    );
    telemetry.record_duration(
        "codex.guardian.review.reconsideration_duration_ms",
        started.elapsed(),
        &tags,
    );
    if assessment.is_none() && !matches!(second.outcome, GuardianReviewSessionOutcome::Aborted) {
        // Do not feed a failed confirmation into the ordinary error retry loop,
        // which could otherwise reconsider the same denial repeatedly.
        second.outcome = first.outcome;
        second.disposition = SessionDisposition::Discard;
        second.turn_completed |= first.turn_completed;
    }
    second
}
