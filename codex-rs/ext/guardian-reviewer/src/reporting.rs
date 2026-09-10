//! Owns review reporting and the per-thread denial window. The host publishes
//! events and enforces interruption only on the still-active owning turn.

use std::sync::Arc;

use codex_analytics::AnalyticsEventsClient;
use codex_analytics::GuardianApprovalRequestSource;
use codex_analytics::GuardianReviewAnalyticsResult;
use codex_analytics::GuardianReviewTrackContext;
use codex_analytics::GuardianReviewedAction;
use codex_extension_api::ExtensionData;
use codex_otel::SessionTelemetry;
use codex_protocol::approvals::GuardianAssessmentAction;
use codex_protocol::approvals::GuardianReviewReason;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::GuardianAssessmentDecisionSource;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::GuardianAssessmentStatus;
use tokio::sync::Mutex;

use crate::GuardianRejectionCircuitBreaker;
use crate::GuardianRejectionCircuitBreakerAction;
use crate::GuardianRejectionCircuitBreakerPolicy;
use crate::GuardianReviewOutcome;
use crate::ReviewCompletion;

/// Captured action attribution used for both the started and terminal report.
pub struct ReviewMetadata {
    pub thread_id: String,
    pub turn_id: String,
    pub review_id: String,
    pub target_item_id: Option<String>,
    pub plugin_id: Option<String>,
    pub script_path: Option<String>,
    pub approval_request_source: GuardianApprovalRequestSource,
    pub reviewed_action: GuardianReviewedAction,
    pub action: GuardianAssessmentAction,
    pub review_reason: GuardianReviewReason,
}

pub struct ReviewReport {
    started: GuardianAssessmentEvent,
    tracking: GuardianReviewTrackContext,
    approval_request_source: GuardianApprovalRequestSource,
    reviewed_action: GuardianReviewedAction,
}

impl ReviewReport {
    pub fn new(metadata: ReviewMetadata) -> Self {
        let tracking = GuardianReviewTrackContext::new(
            metadata.thread_id,
            metadata.turn_id.clone(),
            metadata.review_id.clone(),
            metadata.target_item_id.clone(),
            metadata.approval_request_source,
            metadata.reviewed_action.clone(),
            crate::REVIEW_TIMEOUT.as_millis() as u64,
        );
        let started = GuardianAssessmentEvent {
            review_reason: Some(metadata.review_reason),
            id: metadata.review_id,
            target_item_id: metadata.target_item_id,
            plugin_id: metadata.plugin_id,
            script_path: metadata.script_path,
            turn_id: metadata.turn_id,
            started_at_ms: tracking.started_at_ms.try_into().unwrap_or_default(),
            completed_at_ms: None,
            status: GuardianAssessmentStatus::InProgress,
            risk_level: None,
            user_authorization: None,
            rationale: None,
            decision_source: None,
            action: metadata.action,
        };
        Self {
            started,
            tracking,
            approval_request_source: metadata.approval_request_source,
            reviewed_action: metadata.reviewed_action,
        }
    }

    pub fn turn_id(&self) -> &str {
        &self.started.turn_id
    }

    pub fn started_event(&self) -> GuardianAssessmentEvent {
        self.started.clone()
    }

    pub fn complete(
        &self,
        outcome: GuardianReviewOutcome,
        model: &ModelInfo,
        require_guardian: bool,
        analytics: GuardianReviewAnalyticsResult,
        completed_at_ms: i64,
    ) -> ReviewCompletion {
        let mut event = self.started.clone();
        event.completed_at_ms = Some(completed_at_ms);
        event.decision_source = Some(GuardianAssessmentDecisionSource::Agent);
        crate::complete_review(outcome, model, require_guardian, event, analytics)
    }

    pub fn track(
        &self,
        telemetry: &SessionTelemetry,
        analytics: &AnalyticsEventsClient,
        result: GuardianReviewAnalyticsResult,
        completed_at_ms: u64,
    ) {
        crate::metrics::emit_guardian_review_metrics(
            telemetry,
            &result,
            self.approval_request_source,
            &self.reviewed_action,
            completed_at_ms.saturating_sub(self.tracking.started_at_ms),
        );
        analytics.track_guardian_review(&self.tracking, result, completed_at_ms);
    }
}

/// Thread-scoped accounting shared by synchronous and cached approvals.
#[derive(Default)]
pub struct ReviewDenials(Mutex<GuardianRejectionCircuitBreaker>);

impl ReviewDenials {
    pub fn for_thread(store: &ExtensionData) -> Arc<Self> {
        store.get_or_init(Self::default)
    }

    pub async fn clear_turn(store: &ExtensionData, turn_id: &str) {
        if let Some(state) = store.get::<Self>() {
            state.0.lock().await.clear_turn(turn_id);
        }
    }

    pub async fn record_non_denial(&self, turn_id: &str) {
        self.0.lock().await.record_non_denial(turn_id);
    }

    /// Returns a warning exactly once when this turn reaches its denial limit.
    pub async fn record_denial(&self, turn_id: &str, model: &ModelInfo) -> Option<String> {
        let action = self
            .0
            .lock()
            .await
            .record_denial(turn_id, GuardianRejectionCircuitBreakerPolicy::from(model));
        match action {
            GuardianRejectionCircuitBreakerAction::Continue => None,
            GuardianRejectionCircuitBreakerAction::InterruptTurn {
                consecutive_denials,
                recent_denials,
            } => {
                let window = crate::AUTO_REVIEW_DENIAL_WINDOW_SIZE;
                Some(format!(
                    "Automatic approval review rejected too many approval requests for this turn ({consecutive_denials} consecutive, {recent_denials} in the last {window} reviews); interrupting the turn."
                ))
            }
        }
    }
}
