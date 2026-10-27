//! Exercises async approval and authorization at the host boundary while cleanup is gated.

use super::*;
use crate::ReviewMetadata;
use codex_analytics::AnalyticsEventsClient;
use codex_analytics::GuardianApprovalRequestSource;
use codex_analytics::GuardianReviewedAction;
use codex_extension_api::ExtensionData;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::approvals::GuardianAssessmentAction;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::items::ModelInvocationContext;
use codex_protocol::openai_models::GuardianScope;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SessionSource;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::sync::Notify;
use tokio::sync::Semaphore;

#[derive(Clone)]
struct GatedHost {
    started: Arc<Notify>,
    cleaning: Arc<Notify>,
    cleanup: Arc<Semaphore>,
    cleaned: Arc<AtomicBool>,
    evidence_recorded: Arc<AtomicBool>,
    current: bool,
    validations: Arc<AtomicUsize>,
}

impl ReviewHost for GatedHost {
    type Prepared = ();
    type Evidence = ();

    fn permissions(&self) -> Option<codex_guardian_context::PermissionContext> {
        None
    }
    async fn servicing_turn(&self) -> Option<(String, Arc<ModelInfo>)> {
        None
    }
    fn validate_action(&self) -> Result<(&str, Option<&str>), ReviewDecision> {
        Ok(("turn", Some("call")))
    }
    async fn prepare(
        &self,
        approval_id: &str,
        reason: GuardianReviewReason,
        _: ReviewDeadline,
        _: &CancellationToken,
    ) -> Result<((), ReviewReport), ReviewDecision> {
        Ok((
            (),
            ReviewReport::new(ReviewMetadata {
                thread_id: "thread".into(),
                turn_id: "turn".into(),
                review_id: approval_id.into(),
                target_item_id: Some("call".into()),
                plugin_id: None,
                script_path: None,
                approval_request_source: GuardianApprovalRequestSource::MainTurn,
                reviewed_action: GuardianReviewedAction::RequestPermissions {},
                action: Some(GuardianAssessmentAction::RequestPermissions {
                    reason: None,
                    permissions: Default::default(),
                }),
                review_reason: reason,
                timeout: Some(crate::REVIEW_TIMEOUT),
                model_context: ModelInvocationContext {
                    model_slug: "test".into(),
                    reasoning_effort: None,
                },
            }),
        ))
    }
    async fn attempt(
        &self,
        _: &(),
        _: ReviewDeadline,
        cancellation: &CancellationToken,
    ) -> (
        GuardianReviewOutcome,
        GuardianReviewAnalyticsResult,
        Option<()>,
    ) {
        self.started.notify_one();
        cancellation.cancelled().await;
        self.cleaning.notify_one();
        let _permit = self.cleanup.acquire().await.unwrap();
        self.cleaned.store(true, Ordering::SeqCst);
        // A terminal verdict racing with cancellation must not publish late evidence.
        (
            GuardianReviewOutcome::Completed(
                crate::parse_guardian_assessment(r#"{"outcome":"deny"}"#).unwrap(),
            ),
            GuardianReviewAnalyticsResult::without_session(),
            Some(()),
        )
    }
    async fn cached_approval_is_current(&self, _: &()) -> bool {
        self.validations.fetch_add(/*val*/ 1, Ordering::SeqCst);
        self.current
    }
    async fn emit(&self, _: EventMsg) {}
    async fn record_evidence(&self, _: &(), _: (), _: &GuardianAssessmentEvent) {
        self.evidence_recorded.store(true, Ordering::SeqCst);
    }
    async fn interrupt(&self, _: &str, _: EventMsg, _: ErrorEvent) {
        panic!("cached approval must not interrupt the parent");
    }
}

#[derive(Clone, Copy)]
enum ApprovalState {
    Current,
    Invalidated,
    Cancelled,
}

#[test_case::test_case(ApprovalState::Current; "current action")]
#[test_case::test_case(ApprovalState::Invalidated; "invalidated action")]
#[test_case::test_case(ApprovalState::Cancelled; "cancelled action")]
#[tokio::test]
async fn async_approval_revalidates_without_waiting_for_tracked_cleanup(state: ApprovalState) {
    let host = GatedHost {
        started: Arc::new(Notify::new()),
        cleaning: Arc::new(Notify::new()),
        cleanup: Arc::new(Semaphore::new(/*permits*/ 0)),
        cleaned: Arc::new(AtomicBool::new(/*v*/ false)),
        evidence_recorded: Arc::new(AtomicBool::new(/*v*/ false)),
        current: !matches!(state, ApprovalState::Invalidated),
        validations: Arc::new(AtomicUsize::new(/*v*/ 0)),
    };
    let thread_id = ThreadId::new();
    let store = ExtensionData::new(thread_id.to_string());
    let runtime = store.get_or_init(crate::ReviewerTasks::default);
    let model: ModelInfo = serde_json::from_value(json!({
        "slug":"test", "display_name":"test", "supported_reasoning_levels":[],
        "shell_type":"unified_exec", "visibility":"list", "supported_in_api":true,
        "priority":0, "support_verbosity":false,
        "truncation_policy":{"mode":"bytes","limit":10000}, "experimental_supported_tools":[]
    }))
    .unwrap();
    let telemetry = SessionTelemetry::new(
        thread_id,
        "test",
        "test",
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "test".into(),
        /*log_user_prompts*/ false,
        "test".into(),
        SessionSource::Exec,
    );
    let analytics = AnalyticsEventsClient::disabled();
    let action = json!({});
    let request = ReviewRequest {
        host: host.clone(),
        approval_id: "approval",
        tool_call_id: Some("call"),
        action: Some(&action),
        thread_id,
        thread_store: &store,
        category: GuardianScope::Permissions,
        approval_policy: AskForApproval::OnRequest,
        approvals_reviewer: ApprovalsReviewer::AutoReview,
        require_guardian: false,
        require_synchronous_review: false,
        model_requires_review: false,
        async_enabled: true,
        retried: false,
        escalated_exec: false,
        full_access: false,
        cancellation: CancellationToken::new(),
        timeout: Some(crate::REVIEW_TIMEOUT),
        model: &model,
        telemetry: &telemetry,
        log_assessments: false,
        analytics: &analytics,
        metrics: None,
    };
    let decision = request.review(
        GuardianReviewReason::MissingScore,
        Some(Box::pin(async {
            host.started.notified().await;
            if matches!(state, ApprovalState::Cancelled) {
                request.cancellation.cancel();
            }
        })),
    );
    tokio::pin!(decision);
    let decision = tokio::select! {
        biased;
        decision = &mut decision => decision,
        () = host.cleaning.notified() => panic!("async decision waited for the cancelled review to drain"),
    };
    assert_eq!(
        decision,
        Some(match state {
            ApprovalState::Current => ReviewDecision::Approved,
            ApprovalState::Invalidated | ApprovalState::Cancelled => ReviewDecision::Abort,
        })
    );
    assert_eq!(host.validations.load(Ordering::SeqCst), 1);
    host.cleaning.notified().await;
    assert!(!host.cleaned.load(Ordering::SeqCst));
    assert_eq!(runtime.tasks.len(), 1);
    runtime.tasks.close();
    host.cleanup.add_permits(/*n*/ 1);
    runtime.tasks.wait().await;
    assert!(host.cleaned.load(Ordering::SeqCst));
    assert!(!host.evidence_recorded.load(Ordering::SeqCst));
}
