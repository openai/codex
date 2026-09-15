//! Resolves approval contributors and the synchronous fallback in one place.
//! Cached approvals still validate the bound action before recording their outcome.

use crate::ReviewDenials;
use crate::ReviewHost;
use codex_analytics::AnalyticsEventsClient;
use codex_extension_api::ApprovalDecision;
use codex_extension_api::ApprovalDecisionInput;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionMetrics;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::GuardianV2Enabled;
use codex_extension_api::SynchronousApprovalReviewer;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::approvals::GuardianReviewReason;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::openai_models::GuardianScope;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// One captured action and its host-enforced constraints. Guardian derives the
/// contributor input and owns cancellation for the complete review operation.
pub struct ReviewRequest<'a, H> {
    pub host: H,
    pub approval_id: &'a str,
    pub tool_call_id: Option<&'a str>,
    /// Absent when the host could not render the action for review.
    pub action: Option<&'a serde_json::Value>,
    pub thread_id: ThreadId,
    pub thread_store: &'a ExtensionData,
    pub category: GuardianScope,
    pub approval_policy: AskForApproval,
    pub approvals_reviewer: ApprovalsReviewer,
    pub require_guardian: bool,
    pub require_synchronous_review: bool,
    pub model_requires_review: bool,
    pub async_enabled: bool,
    pub retried: bool,
    pub escalated_exec: bool,
    pub full_access: bool,
    pub cancellation: CancellationToken,
    pub model: &'a ModelInfo,
    pub telemetry: &'a SessionTelemetry,
    pub analytics: &'a AnalyticsEventsClient,
    pub metrics: Option<Arc<dyn ExtensionMetrics>>,
}

pub fn routes_approval_policy_to_guardian(
    policy: AskForApproval,
    reviewer: ApprovalsReviewer,
) -> bool {
    matches!(
        policy,
        AskForApproval::OnRequest | AskForApproval::Granular(_)
    ) && reviewer == ApprovalsReviewer::AutoReview
}

impl<H: ReviewHost> ReviewRequest<'_, H> {
    pub async fn decide<C: Sync>(
        mut self,
        registry: &ExtensionRegistry<C>,
    ) -> Option<ReviewDecision> {
        let runtime = self.thread_store.get::<crate::ReviewerTasks>();
        let _task = runtime.as_ref().map(|runtime| runtime.tasks.token());
        if runtime
            .as_ref()
            .is_some_and(|runtime| runtime.cancellation.is_cancelled())
        {
            return Some(ReviewDecision::Abort);
        }
        let cancellation = self.cancellation.child_token();
        let _cancel_on_drop = cancellation.clone().drop_guard();
        self.cancellation = cancellation.clone();
        let decision = async {
            if self.full_access {
                return Some(match self.host.validate_action() {
                    Ok(_) => ReviewDecision::Approved,
                    Err(decision) => decision,
                });
            }
            let Some(action) = self.action else {
                return Some(ReviewDecision::denied(
                    "automatic approval review could not prepare the action",
                ));
            };
            let input = ApprovalDecisionInput {
                approval_id: self.approval_id,
                tool_call_id: self.tool_call_id,
                action,
                thread_id: self.thread_id,
                thread_store: self.thread_store,
                category: self.category,
                approval_policy: self.approval_policy,
                approvals_reviewer: self.approvals_reviewer,
                require_guardian: self.require_guardian,
                require_fresh_review: self.require_synchronous_review
                    || self.model_requires_review && !self.async_enabled
                    || self.cancellation.is_cancelled()
                    || self.retried
                    || self.escalated_exec,
                full_access: self.full_access,
                metrics: self.metrics.clone(),
                synchronous_reviewer: &self,
            };
            match registry.decide_approval(&input).await {
                Some(ApprovalDecision::Reviewed(decision)) => Some(decision),
                Some(ApprovalDecision::Allow) if !input.require_fresh_review => {
                    Some(self.cached_approval().await)
                }
                Some(ApprovalDecision::Allow) => {
                    self.review(GuardianReviewReason::FreshRequired).await
                }
                Some(ApprovalDecision::AskUser) if !self.require_guardian => None,
                None if !self.require_guardian
                    && !routes_approval_policy_to_guardian(
                        self.approval_policy,
                        self.approvals_reviewer,
                    ) =>
                {
                    None
                }
                None | Some(ApprovalDecision::AskUser) => {
                    self.review(GuardianReviewReason::Policy).await
                }
            }
        };
        tokio::pin!(decision);
        let result = if let Some(runtime) = runtime.as_ref() {
            tokio::select! {
                biased;
                _ = runtime.cancellation.cancelled() => {
                    cancellation.cancel();
                    // Drive cancellation through reporting and agent cleanup before releasing
                    // the tracked operation. Parent stop joins this work before closing history.
                    decision.await
                }
                result = &mut decision => result,
            }
        } else {
            decision.await
        };
        if cancellation.is_cancelled()
            || runtime
                .as_ref()
                .is_some_and(|runtime| runtime.cancellation.is_cancelled())
        {
            Some(ReviewDecision::Abort)
        } else {
            result
        }
    }

    async fn cached_approval(&self) -> ReviewDecision {
        let (turn_id, item_id) = match self.host.validate_action() {
            Ok(target) => target,
            Err(decision) => return decision,
        };
        if self.cancellation.is_cancelled() {
            return ReviewDecision::Abort;
        }
        if self.thread_store.get::<GuardianV2Enabled>().is_some() {
            self.analytics
                .track_guardian_v2_event(codex_analytics::GuardianV2Event {
                    thread_id: self.thread_id.to_string(),
                    turn_id: turn_id.to_owned(),
                    item_id: item_id.map(str::to_owned),
                    model: Some(self.model.slug.clone()),
                    occurred_at_ms: codex_analytics::now_unix_millis(),
                    kind: codex_analytics::GuardianV2EventKind::FastDecision {
                        decision: "approved",
                    },
                });
        }
        if let Some((turn_id, _)) = self.host.servicing_turn().await {
            ReviewDenials::for_thread(self.thread_store)
                .record_non_denial(&turn_id)
                .await;
        }
        ReviewDecision::Approved
    }
}
