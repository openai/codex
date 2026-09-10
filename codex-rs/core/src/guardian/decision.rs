//! Calls the decision extension for each approval and enforces host constraints.
//! The synchronous service captures one action; no outcome is stored by tool-call ID.

use codex_async_utils::THREAD_STACK_SIZE_BYTES;
use codex_extension_api::ApprovalDecision;
use codex_extension_api::ApprovalDecisionInput;
use codex_extension_api::SynchronousApprovalReviewer;
use codex_protocol::approvals::GuardianReviewReason;
use codex_protocol::protocol::ReviewDecision;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::ApprovalRequestReasons;
use super::GuardianApprovalRequest;
use super::GuardianReviewContext;
use super::GuardianReviewOptions;
use super::review::record_guardian_non_denial;
use super::runtime::ReviewAction;
use super::runtime::ReviewRuntime;
use crate::session::session::Session;

pub(crate) fn spawn_approval_decision(
    session: Arc<Session>,
    context: impl Into<GuardianReviewContext>,
    review_id: String,
    request: impl Into<super::ReviewAction>,
    reasons: ApprovalRequestReasons,
    options: GuardianReviewOptions,
) -> oneshot::Receiver<Option<ReviewDecision>> {
    let context: GuardianReviewContext = context.into();
    let request: ReviewAction = request.into();
    let (tx, rx) = oneshot::channel();
    let runtime = session.services.runtime_handle.clone();
    let spawn_result = std::thread::Builder::new()
        .name("codex-approval-review".to_string())
        .stack_size(THREAD_STACK_SIZE_BYTES)
        .spawn(move || {
            let decision = runtime.block_on(decide_approval(
                session, context, review_id, request, reasons, options,
            ));
            let _ = tx.send(decision);
        });
    if let Err(err) = spawn_result {
        tracing::error!(%err, "failed to spawn automatic approval review worker");
    }
    rx
}

/// `None` requests the existing user flow. No contributor is never an implicit allow.
pub(crate) async fn decide_approval(
    session: Arc<Session>,
    context: impl Into<GuardianReviewContext>,
    review_id: String,
    request: impl Into<ReviewAction>,
    reasons: ApprovalRequestReasons,
    options: GuardianReviewOptions,
) -> Option<ReviewDecision> {
    let context = context.into();
    let request = request.into();
    let turn = context.turn();
    let live_config = session.get_config().await;
    let requirements = live_config.config_layer_stack.requirements();
    let model_requires_review =
        requirements.auto_review_required_for_model(&context.model_info.slug);
    let require_guardian = options.require_guardian
        || model_requires_review
        || requirements
            .approvals_reviewer
            .can_set(&codex_protocol::config_types::ApprovalsReviewer::User)
            .is_err();
    let require_fresh_review = options.require_synchronous_review
        || model_requires_review
            && !turn
                .config
                .features
                .enabled(codex_features::Feature::GuardianV2)
        || options
            .external_cancel
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        || reasons.retry.is_some()
        || matches!(&request.request, Ok(GuardianApprovalRequest::ExecCommand { sandbox_permissions, .. })
            if sandbox_permissions.requires_escalated_permissions());
    let full_access = context.environments().has_full_access(
        context.approval_policy,
        &turn.config.permissions.effective_permission_profile(),
    );
    if full_access {
        return Some(if let Err(decision) = request.validate(&context) {
            decision
        } else if options
            .external_cancel
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            ReviewDecision::Abort
        } else {
            ReviewDecision::Approved
        });
    }
    let action = match &request.action {
        Ok(action) => action,
        Err(_) => {
            return Some(ReviewDecision::denied(
                "automatic approval review could not prepare the action",
            ));
        }
    };
    let runtime = codex_guardian_reviewer::SynchronousReview::new(ReviewRuntime {
        session: Arc::clone(&session),
        context: context.clone(),
        review_id: review_id.clone(),
        request: request.clone(),
        reasons,
        options: GuardianReviewOptions {
            require_guardian,
            ..options
        },
    });
    let input = ApprovalDecisionInput {
        approval_id: &review_id,
        tool_call_id: request
            .request
            .as_ref()
            .ok()
            .and_then(|request| match request {
                // Stdin's target is the terminal launch; freshness belongs to this write.
                GuardianApprovalRequest::WriteStdin { approval_id, .. } => {
                    Some(approval_id.as_str())
                }
                // Intercepts retain the launch ID, not the current triggering call.
                #[cfg(unix)]
                GuardianApprovalRequest::Execve { .. } => None,
                _ => super::approval_request::guardian_request_target_item_id(request),
            }),
        action,
        thread_id: session.thread_id,
        thread_store: &session.services.thread_extension_data,
        category: request.category,
        approval_policy: context.approval_policy,
        approvals_reviewer: context.approvals_reviewer,
        require_guardian,
        require_fresh_review,
        full_access,
        metrics: Some(crate::session::extension_metrics::from_session_telemetry(
            turn.session_telemetry.clone(),
        )),
        synchronous_reviewer: &runtime,
    };
    match session.services.extensions.decide_approval(&input).await {
        Some(ApprovalDecision::Reviewed(decision)) => Some(decision),
        Some(ApprovalDecision::Allow) if !require_fresh_review => {
            let request = match request.validate(&context) {
                Ok(request) => request,
                Err(decision) => return Some(decision),
            };
            let turn_id = super::approval_request::guardian_request_turn_id(request, &turn.sub_id);
            if session
                .services
                .thread_extension_data
                .get::<codex_extension_api::GuardianV2Enabled>()
                .is_some()
            {
                session
                    .services
                    .analytics_events_client
                    .track_guardian_v2_event(codex_analytics::GuardianV2Event {
                        thread_id: session.thread_id.to_string(),
                        turn_id: turn_id.to_owned(),
                        item_id: super::approval_request::guardian_request_target_item_id(request)
                            .map(str::to_owned),
                        model: Some(turn.model_info().slug.clone()),
                        occurred_at_ms: codex_analytics::now_unix_millis(),
                        kind: codex_analytics::GuardianV2EventKind::FastDecision {
                            decision: "approved",
                        },
                    });
            }
            record_guardian_non_denial(&session, turn_id).await;
            Some(ReviewDecision::Approved)
        }
        Some(ApprovalDecision::Allow) => runtime.review(GuardianReviewReason::FreshRequired).await,
        Some(ApprovalDecision::AskUser) if !require_guardian => None,
        None if !require_guardian
            && !super::review::routes_approval_policy_to_guardian(
                context.approval_policy,
                context.approvals_reviewer,
            ) =>
        {
            None
        }
        None | Some(ApprovalDecision::AskUser) => {
            runtime.review(GuardianReviewReason::Policy).await
        }
    }
}
