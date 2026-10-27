//! Owns synchronous review orchestration, reporting and outcome accounting.
//! The host binds the original action, captures evidence and enforces authorization.
//! Async approvals can return while cancelled attempts finish under the parent's task tracker.

use crate::GuardianReviewError;
use crate::GuardianReviewOutcome;
use crate::GuardianReviewSessionLimits;
use crate::ReviewDeadline;
use crate::ReviewDenials;
use crate::ReviewReport;
use crate::ReviewRequest;
use codex_analytics::GuardianReviewAnalyticsResult;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::SynchronousApprovalReviewer;
use codex_protocol::approvals::GuardianReviewReason;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::GuardianAssessmentOutcome;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::WarningEvent;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use tokio_util::sync::CancellationToken;

/// Operations bound to one immutable action and its issuing context.
/// Hosts validate authority, publish supplied events and interrupt only the selected turn;
/// they do not select Guardian outcomes, retry policy or reporting effects.
/// Clones retain the same action and context. A cancelled attempt may keep its host and
/// prepared state alive under the parent's task tracker after approval returns.
pub trait ReviewHost: Clone + Send + Sync + 'static {
    type Prepared: Send + Sync + 'static;
    /// Evidence captured for one completed assessment, never reused by another attempt.
    type Evidence: Send + 'static;
    /// Returns the target's rules, or no evidence when they cannot be resolved.
    fn permissions(&self) -> Option<codex_guardian_context::PermissionContext>;
    /// Captures the turn currently servicing reviews, which may differ from a yielded cell's origin.
    fn servicing_turn(&self) -> impl Future<Output = Option<(String, Arc<ModelInfo>)>> + Send;
    /// Returns the owning turn and optional target item after validating the action.
    fn validate_action(&self) -> Result<(&str, Option<&str>), ReviewDecision>;
    /// Prepares evidence and accounting, with a client event only for supported actions.
    fn prepare(
        &self,
        approval_id: &str,
        reason: GuardianReviewReason,
        deadline: ReviewDeadline,
        cancellation: &CancellationToken,
    ) -> impl Future<Output = Result<(Self::Prepared, ReviewReport), ReviewDecision>> + Send;
    /// Captures fresh authorization evidence and rejects stale approvals for each attempt.
    fn attempt(
        &self,
        prepared: &Self::Prepared,
        deadline: ReviewDeadline,
        cancellation: &CancellationToken,
    ) -> impl Future<
        Output = (
            GuardianReviewOutcome,
            GuardianReviewAnalyticsResult,
            Option<Self::Evidence>,
        ),
    > + Send;
    /// Revalidates the original action and history lifetime before using cached approval.
    fn cached_approval_is_current(
        &self,
        prepared: &Self::Prepared,
    ) -> impl Future<Output = bool> + Send;
    fn emit(&self, event: EventMsg) -> impl Future<Output = ()> + Send;
    fn record_evidence(
        &self,
        prepared: &Self::Prepared,
        evidence: Self::Evidence,
        event: &GuardianAssessmentEvent,
    ) -> impl Future<Output = ()> + Send;
    fn interrupt(
        &self,
        turn_id: &str,
        warning: EventMsg,
        error: ErrorEvent,
    ) -> impl Future<Output = ()> + Send;
}

impl<H: ReviewHost> SynchronousApprovalReviewer for ReviewRequest<'_, H> {
    #[tracing::instrument(
        name = "guardian_review",
        level = "debug",
        skip_all,
        fields(
            review_id = self.approval_id,
            thread_id = %self.thread_id,
            tool_call_id = self.tool_call_id,
        )
    )]
    fn review<'a>(
        &'a self,
        reason: GuardianReviewReason,
        async_approval: Option<ExtensionFuture<'a, ()>>,
    ) -> ExtensionFuture<'a, Option<ReviewDecision>> {
        Box::pin(async move {
            if async_approval.is_none() {
                self.telemetry.counter(
                    "codex.guardian.review.route",
                    /*inc*/ 1,
                    &[("route", "required_sync"), ("reason", reason.into())],
                );
            }
            let deadline = ReviewDeadline::from_timeout(self.timeout);
            let (prepared, report) = match self
                .host
                .prepare(self.approval_id, reason, deadline, &self.cancellation)
                .await
            {
                Ok(prepared) => prepared,
                Err(decision) => return Some(decision),
            };
            if let Some(event) = report.started_event() {
                self.host.emit(EventMsg::GuardianAssessment(event)).await;
            }
            let prepared = Arc::new(prepared);
            let cancellation = self.cancellation.child_token();
            let attempts = Arc::new(AtomicI64::new(/*v*/ 0));
            let mut review = Box::pin({
                let host = self.host.clone();
                let prepared = Arc::clone(&prepared);
                let cancellation = cancellation.clone();
                let attempts = Arc::clone(&attempts);
                let telemetry = self.telemetry.clone();
                async move {
                    crate::run_with_retry(
                        GuardianReviewSessionLimits {
                            max_attempts: crate::MAX_REVIEW_ATTEMPTS,
                            deadline,
                        },
                        Some(&cancellation),
                        Some(&telemetry),
                        |deadline| {
                            attempts.fetch_add(/*val*/ 1, Ordering::Relaxed);
                            host.attempt(&prepared, deadline, &cancellation)
                        },
                    )
                    .await
                }
            });
            let (outcome, analytics, evidence) = if let Some(async_approval) = async_approval {
                tokio::select! {
                    biased;
                    result = &mut review => result,
                    () = async_approval => {
                        cancellation.cancel();
                        let analytics = if let Some(runtime) = self.thread_store.get::<crate::ReviewerTasks>() {
                            // Keep driving the same attempt: its pool reservation stays held
                            // until it drains, and parent shutdown joins the tracked cleanup.
                            runtime.tasks.spawn(async move { let _ = review.await; });
                            // Final session and usage metadata are not available yet.
                            GuardianReviewAnalyticsResult {
                                attempt_count: attempts.load(Ordering::Relaxed),
                                ..GuardianReviewAnalyticsResult::without_session()
                            }
                        } else {
                            // Hosts without a lifetime tracker must finish cleanup here.
                            let (_, analytics, _) = review.await;
                            analytics
                        };
                        let outcome = if !self.host.cached_approval_is_current(&prepared).await
                            || self.cancellation.is_cancelled() {
                            GuardianReviewOutcome::Error(GuardianReviewError::Cancelled)
                        } else if deadline.is_elapsed() {
                            GuardianReviewOutcome::Error(GuardianReviewError::Timeout)
                        } else {
                            GuardianReviewOutcome::CachedApproval
                        };
                        (outcome, analytics, None)
                    }
                }
            } else {
                review.await
            };
            let completed_at_ms = codex_analytics::now_unix_millis();
            let cached_approval = matches!(outcome, GuardianReviewOutcome::CachedApproval);
            let (completed, elapsed) = report.complete(
                outcome,
                self.model,
                self.require_guardian,
                analytics,
                completed_at_ms.try_into().unwrap_or_default(),
            );
            if self.log_assessments
                && !cached_approval
                && let Some(event) = &completed.event
            {
                self.telemetry
                    .guardian_assessment(event, completed.assessment_outcome);
            }
            report.track(
                self.telemetry,
                self.analytics,
                completed.analytics,
                completed_at_ms,
                elapsed,
            );
            if let Some(message) = completed.warning {
                self.host
                    .emit(EventMsg::GuardianWarning(WarningEvent { message }))
                    .await;
            }
            if completed.assessment_outcome.is_some()
                && let Some(evidence) = evidence
                && let Some(event) = &completed.event
            {
                self.host.record_evidence(&prepared, evidence, event).await;
            }
            if let Some(event) = completed.event {
                self.host.emit(EventMsg::GuardianAssessment(event)).await;
            }
            if cached_approval {
                return Some(self.cached_approval().await);
            }
            if let Some((turn_id, model)) = self.host.servicing_turn().await {
                let denials = ReviewDenials::for_thread(self.thread_store);
                if completed.assessment_outcome == Some(GuardianAssessmentOutcome::Deny) {
                    if let Some(message) = denials.record_denial(&turn_id, &model).await {
                        // The denial window returns a warning only once per turn.
                        self.telemetry.counter(
                            "codex.guardian.denial_limit_reached",
                            /*inc*/ 1,
                            &[],
                        );
                        self.host
                            .interrupt(
                                &turn_id,
                                EventMsg::GuardianWarning(WarningEvent {
                                    message: message.clone(),
                                }),
                                ErrorEvent {
                                    message,
                                    codex_error_info: Some(CodexErrorInfo::TooManyDenials),
                                    misalignment: None,
                                },
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

#[cfg(test)]
#[path = "review_tests.rs"]
mod tests;
