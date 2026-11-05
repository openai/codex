//! Selects a reviewer response before it enters history. A terminal denial is
//! resampled once; failures retain that denial and cancellation still aborts.

use crate::GuardianReviewResponse;
use crate::GuardianReviewSessionOutcome;
use crate::ReviewTurnResult;
use crate::SessionDisposition;
use codex_otel::SessionTelemetry;
use codex_protocol::protocol::GuardianAssessmentOutcome;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::time::Instant;

struct ProvisionalDenial {
    response: GuardianReviewResponse,
    started: Instant,
}

/// Review-scoped selection shared by sampling and the deadline/cancellation owner.
#[derive(Default)]
pub struct DenialResample {
    denial: Mutex<Option<ProvisionalDenial>>,
}

impl DenialResample {
    /// Resamples the first terminal denial. Tool calls must proceed normally.
    pub fn should_resample(&self, response: GuardianReviewResponse) -> bool {
        let mut denial = self.denial.lock().unwrap_or_else(PoisonError::into_inner);
        if denial.is_none()
            && response
                .assessment()
                .is_ok_and(|assessment| assessment.outcome == GuardianAssessmentOutcome::Deny)
        {
            *denial = Some(ProvisionalDenial {
                response,
                started: Instant::now(),
            });
            true
        } else {
            false
        }
    }

    /// Completes accounting and preserves the denial if the retry could not finish.
    pub fn finish(
        &self,
        mut result: ReviewTurnResult,
        telemetry: &SessionTelemetry,
    ) -> ReviewTurnResult {
        let Some(first) = self
            .denial
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        else {
            return result;
        };
        let assessment = match &result.outcome {
            GuardianReviewSessionOutcome::Completed(Ok(Some(response))) => {
                response.assessment().ok()
            }
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
            None => match &result.outcome {
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
            first.started.elapsed(),
            &tags,
        );
        if assessment.is_none() && !matches!(result.outcome, GuardianReviewSessionOutcome::Aborted)
        {
            result.outcome = GuardianReviewSessionOutcome::Completed(Ok(Some(first.response)));
            // Preserve the last committed reviewer context, without retaining a failed retry.
            if result.disposition == SessionDisposition::Reusable {
                result.disposition = SessionDisposition::Rollback;
            }
        }
        result
    }
}
