//! Orchestrates one synchronous approval review over a host-bound action.
//! All callers share this policy; the host captures evidence and enforces the result.

use std::future::Future;

use codex_analytics::GuardianReviewAnalyticsResult;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::SynchronousApprovalReviewer;
use codex_protocol::approvals::GuardianReviewReason;
use codex_protocol::protocol::ReviewDecision;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::GuardianReviewOutcome;
use crate::GuardianReviewSessionLimits;

/// Runtime operations bound to one immutable approval action and its issuing context.
/// Preparation captures trusted evidence. Completion must invalidate stale approvals
/// and may only satisfy the review gate, never expand the action's execution authority.
/// Completion may return `None` for user approval only when host policy permits it.
pub trait ReviewHost: Send + Sync {
    type Prepared: Send + Sync;

    fn cancellation(&self) -> Option<&CancellationToken>;
    fn prepare(
        &self,
        reason: GuardianReviewReason,
        deadline: Instant,
    ) -> impl Future<Output = Result<Self::Prepared, ReviewDecision>> + Send;
    fn attempt(
        &self,
        prepared: &Self::Prepared,
        deadline: Instant,
    ) -> impl Future<Output = (GuardianReviewOutcome, GuardianReviewAnalyticsResult)> + Send;
    fn complete(
        &self,
        prepared: Self::Prepared,
        outcome: GuardianReviewOutcome,
        analytics: GuardianReviewAnalyticsResult,
    ) -> impl Future<Output = Option<ReviewDecision>> + Send;
}

/// One review bound by the host before Guardian's approval policy chooses to run it.
pub struct SynchronousReview<H> {
    host: H,
}

impl<H: ReviewHost> SynchronousReview<H> {
    pub fn new(host: H) -> Self {
        Self { host }
    }
}

impl<H: ReviewHost> SynchronousApprovalReviewer for SynchronousReview<H> {
    fn review(&self, reason: GuardianReviewReason) -> ExtensionFuture<'_, Option<ReviewDecision>> {
        Box::pin(async move {
            let deadline = Instant::now() + crate::REVIEW_TIMEOUT;
            let prepared = match self.host.prepare(reason, deadline).await {
                Ok(prepared) => prepared,
                Err(decision) => return Some(decision),
            };
            let (outcome, analytics) = Box::pin(crate::run_with_retry(
                GuardianReviewSessionLimits {
                    max_attempts: crate::MAX_REVIEW_ATTEMPTS,
                    deadline,
                },
                self.host.cancellation(),
                |deadline| self.host.attempt(&prepared, deadline),
            ))
            .await;
            self.host.complete(prepared, outcome, analytics).await
        })
    }
}
