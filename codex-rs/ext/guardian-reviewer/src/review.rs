//! Owns synchronous review orchestration, reporting and outcome accounting.
//! The host binds the original action, captures evidence and enforces authorization.

use crate::GuardianReviewError;
use crate::GuardianReviewOutcome;
use crate::GuardianReviewSessionLimits;
use crate::ReviewDenials;
use crate::ReviewReport;
use codex_analytics::AnalyticsEventsClient;
use codex_analytics::GuardianReviewAnalyticsResult;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::SynchronousApprovalReviewer;
use codex_otel::SessionTelemetry;
use codex_protocol::approvals::GuardianReviewReason;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::GuardianAssessmentOutcome;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::WarningEvent;
use std::future::Future;
use std::sync::Arc;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

/// Operations bound to one immutable action and its issuing context.
/// Hosts validate authority, publish supplied events and interrupt only the selected turn;
/// they do not select Guardian outcomes, retry policy or reporting effects.
pub trait ReviewHost: Send + Sync {
    type Prepared: Send + Sync;
    fn cancellation(&self) -> Option<&CancellationToken>;
    /// Captures the turn currently servicing reviews, which may differ from a yielded cell's origin.
    fn servicing_turn(&self) -> impl Future<Output = Option<(String, Arc<ModelInfo>)>> + Send;
    fn prepare(
        &self,
        reason: GuardianReviewReason,
        deadline: Instant,
    ) -> impl Future<Output = Result<(Self::Prepared, ReviewReport), ReviewDecision>> + Send;
    /// Rejects stale approvals before returning the attempt's outcome.
    fn attempt(
        &self,
        prepared: &Self::Prepared,
        deadline: Instant,
    ) -> impl Future<Output = (GuardianReviewOutcome, GuardianReviewAnalyticsResult)> + Send;
    fn emit(&self, event: EventMsg) -> impl Future<Output = ()> + Send;
    fn record_evidence(
        &self,
        prepared: &Self::Prepared,
        event: &GuardianAssessmentEvent,
    ) -> impl Future<Output = ()> + Send;
    fn interrupt(&self, turn_id: &str, warning: EventMsg) -> impl Future<Output = ()> + Send;
}

/// One review bound by the host before Guardian's approval policy chooses to run it.
pub struct SynchronousReview<'a, H> {
    pub host: H,
    pub thread_store: &'a ExtensionData,
    pub model: &'a ModelInfo,
    pub require_guardian: bool,
    pub telemetry: &'a SessionTelemetry,
    pub analytics: &'a AnalyticsEventsClient,
}

impl<H: ReviewHost> SynchronousApprovalReviewer for SynchronousReview<'_, H> {
    fn review(&self, reason: GuardianReviewReason) -> ExtensionFuture<'_, Option<ReviewDecision>> {
        Box::pin(async move {
            let deadline = Instant::now() + crate::REVIEW_TIMEOUT;
            let (context, report) = match self.host.prepare(reason, deadline).await {
                Ok(prepared) => prepared,
                Err(decision) => return Some(decision),
            };
            self.host
                .emit(EventMsg::GuardianAssessment(report.started_event()))
                .await;
            let (outcome, analytics) = if self
                .host
                .cancellation()
                .is_some_and(CancellationToken::is_cancelled)
            {
                (
                    GuardianReviewOutcome::Error(GuardianReviewError::Cancelled),
                    GuardianReviewAnalyticsResult::without_session(),
                )
            } else {
                Box::pin(crate::run_with_retry(
                    GuardianReviewSessionLimits {
                        max_attempts: crate::MAX_REVIEW_ATTEMPTS,
                        deadline,
                    },
                    self.host.cancellation(),
                    |deadline| self.host.attempt(&context, deadline),
                ))
                .await
            };
            let completed_at_ms = codex_analytics::now_unix_millis();
            let completed = report.complete(
                outcome,
                self.model,
                self.require_guardian,
                analytics,
                completed_at_ms.try_into().unwrap_or_default(),
            );
            report.track(
                self.telemetry,
                self.analytics,
                completed.analytics,
                completed_at_ms,
            );
            if let Some(message) = completed.warning {
                self.host
                    .emit(EventMsg::GuardianWarning(WarningEvent { message }))
                    .await;
            }
            if completed.assessment_outcome.is_some() {
                self.host.record_evidence(&context, &completed.event).await;
            }
            self.host
                .emit(EventMsg::GuardianAssessment(completed.event))
                .await;
            if let Some((turn_id, model)) = self.host.servicing_turn().await {
                let denials = ReviewDenials::for_thread(self.thread_store);
                if completed.assessment_outcome == Some(GuardianAssessmentOutcome::Deny) {
                    if let Some(message) = denials.record_denial(&turn_id, &model).await {
                        self.host
                            .interrupt(
                                &turn_id,
                                EventMsg::GuardianWarning(WarningEvent { message }),
                            )
                            .await;
                    }
                } else {
                    denials.record_non_denial(&turn_id).await;
                }
            }
            completed.decision
        })
    }
}
