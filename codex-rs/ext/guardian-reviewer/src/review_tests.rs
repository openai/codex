//! Exercises async approval and recovered denials at the host boundary.

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

const SCORED: CachedApprovalContext = CachedApprovalContext {
    user_message_revision: 2,
    review_context_revision: 3,
    root_versions: Some((
        crate::GuardianAuthorizationVersion {
            root_runtime_id: Some([1; 16]),
            user_message_revision: 1,
            retained_context_complete: true,
        },
        1,
    )),
};

#[derive(Clone)]
struct GatedHost {
    started: Arc<Notify>,
    cleaning: Arc<Notify>,
    cleanup: Arc<Semaphore>,
    cleaned: Arc<AtomicBool>,
    evidence_recorded: Arc<AtomicBool>,
    state: ApprovalState,
    validations: Arc<AtomicUsize>,
}

impl ReviewHost for GatedHost {
    type Prepared = ReviewDeadline;
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
        deadline: ReviewDeadline,
        _: &CancellationToken,
    ) -> Result<(Self::Prepared, ReviewReport), ReviewDecision> {
        // Make a fresh timeout at verdict time distinguishable from the original deadline.
        tokio::time::advance(std::time::Duration::from_secs(1)).await;
        Ok((
            deadline,
            ReviewReport::new(ReviewMetadata {
                thread_id: "thread".into(),
                turn_id: "turn".into(),
                root_turn_id: None,
                turn_trigger: None,
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
        preparation_deadline: &Self::Prepared,
        deadline: ReviewDeadline,
        cancellation: &CancellationToken,
    ) -> (
        GuardianReviewOutcome,
        GuardianReviewAnalyticsResult,
        Option<()>,
    ) {
        assert_eq!(deadline, *preparation_deadline);
        if matches!(self.state, ApprovalState::RecoveredDenial) {
            deadline.wait().await;
        } else {
            self.started.notify_one();
            cancellation.cancelled().await;
            self.cleaning.notify_one();
            let _permit = self.cleanup.acquire().await.unwrap();
            self.cleaned.store(true, Ordering::SeqCst);
        }
        (
            GuardianReviewOutcome::Completed(
                crate::parse_guardian_assessment(
                    r#"{"outcome":"deny","rationale":"First denial"}"#,
                )
                .unwrap(),
            ),
            GuardianReviewAnalyticsResult::without_session(),
            Some(()),
        )
    }
    async fn cached_approval_context(&self, _: &Self::Prepared) -> Option<CachedApprovalContext> {
        self.validations.fetch_add(/*val*/ 1, Ordering::SeqCst);
        let mut current = SCORED;
        let (authorization, context) = current.root_versions.as_mut().unwrap();
        match self.state {
            ApprovalState::Invalidated => return None,
            ApprovalState::Revoked => authorization.user_message_revision += 1,
            ApprovalState::ContextChanged => *context += 1,
            ApprovalState::WorkerRevoked => current.user_message_revision += 1,
            ApprovalState::WorkerContextChanged => current.review_context_revision += 1,
            ApprovalState::Current | ApprovalState::Cancelled | ApprovalState::RecoveredDenial => {}
        }
        Some(current)
    }
    async fn emit(&self, _: EventMsg) {}
    async fn record_evidence(&self, _: &Self::Prepared, _: (), _: &GuardianAssessmentEvent) {
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
    Revoked,
    ContextChanged,
    WorkerRevoked,
    WorkerContextChanged,
    Cancelled,
    RecoveredDenial,
}

#[test_case::test_case(ApprovalState::Current; "current action")]
#[test_case::test_case(ApprovalState::Invalidated; "invalidated action")]
#[test_case::test_case(ApprovalState::Revoked; "root revokes permission after late low")]
#[test_case::test_case(ApprovalState::ContextChanged; "root review context changes after late low")]
#[test_case::test_case(ApprovalState::WorkerRevoked; "worker authorization changes after late low")]
#[test_case::test_case(ApprovalState::WorkerContextChanged; "worker review context changes after late low")]
#[test_case::test_case(ApprovalState::Cancelled; "cancelled action")]
#[test_case::test_case(ApprovalState::RecoveredDenial; "hosted timeout retains denial")]
#[tokio::test(start_paused = true)]
async fn review_handles_async_approval_and_timeout(state: ApprovalState) {
    let host = GatedHost {
        started: Arc::new(Notify::new()),
        cleaning: Arc::new(Notify::new()),
        cleanup: Arc::new(Semaphore::new(/*permits*/ 0)),
        cleaned: Arc::new(AtomicBool::new(/*v*/ false)),
        evidence_recorded: Arc::new(AtomicBool::new(/*v*/ false)),
        state,
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
        sampling_model: "test",
        turn_id: "turn",
        serving_assessment: None,
        host: host.clone(),
        approval_id: "approval",
        tool_call_id: Some("call"),
        action: Some(&action),
        thread_id,
        thread_store: &store,
        turn_store: &store,
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
        host_deadline: matches!(state, ApprovalState::RecoveredDenial)
            .then(|| ReviewDeadline::from_timeout(Some(crate::REVIEW_TIMEOUT))),
        model: &model,
        telemetry: &telemetry,
        log_assessments: false,
        analytics: &analytics,
        metrics: None,
    };
    let expected = match state {
        ApprovalState::Current => ReviewDecision::Approved,
        ApprovalState::Invalidated
        | ApprovalState::Revoked
        | ApprovalState::ContextChanged
        | ApprovalState::WorkerRevoked
        | ApprovalState::WorkerContextChanged
        | ApprovalState::Cancelled => ReviewDecision::Abort,
        ApprovalState::RecoveredDenial => {
            let decision = request
                .review(
                    GuardianReviewReason::MissingScore,
                    /*async_approval*/ None,
                )
                .await;
            assert!(
                matches!(decision, Some(ReviewDecision::Denied { rejection })
                if rejection.contains("First denial"))
            );
            assert!(host.evidence_recorded.load(Ordering::SeqCst));
            return;
        }
    };
    let decision = request.review(
        GuardianReviewReason::MissingScore,
        Some(Box::pin(async {
            host.started.notified().await;
            if matches!(state, ApprovalState::Cancelled) {
                request.cancellation.cancel();
            }
            SCORED
        })),
    );
    tokio::pin!(decision);
    let decision = tokio::select! {
        biased;
        decision = &mut decision => decision,
        () = host.cleaning.notified() => panic!("async decision waited for the cancelled review to drain"),
    };
    assert_eq!(decision, Some(expected));
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
