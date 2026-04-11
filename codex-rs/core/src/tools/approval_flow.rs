use std::sync::Arc;

use codex_hooks::PermissionRequestDecision;
use codex_hooks::PermissionRequestGuardianReview;
use codex_hooks::PermissionRequestGuardianReviewDecision;
use codex_hooks::PermissionRequestGuardianReviewStatus as HookGuardianReviewStatus;
use codex_hooks::PermissionRequestRequest;
use codex_protocol::protocol::ReviewDecision;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::GuardianApprovalReview;
use crate::guardian::GuardianApprovalReviewResult;
use crate::guardian::GuardianApprovalReviewStatus;
use crate::guardian::GuardianAssessmentOutcome;
use crate::guardian::new_guardian_review_id;
use crate::guardian::review_approval_request_with_review_detached;
use crate::guardian::routes_approval_to_guardian;
use crate::hook_runtime::run_permission_request_hook_request;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PermissionApprovalSource {
    Guardian,
    Hook,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PermissionApprovalOutcome {
    Decision {
        decision: ReviewDecision,
        source: PermissionApprovalSource,
        guardian_review_id: Option<String>,
    },
    HookDenied {
        message: String,
    },
}

pub(crate) async fn run_permission_approval_flow(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    guardian_request: Option<GuardianApprovalRequest>,
    hook_request: Option<PermissionRequestRequest>,
    retry_reason: Option<String>,
) -> Option<PermissionApprovalOutcome> {
    let guardian_review_id = (routes_approval_to_guardian(turn) && guardian_request.is_some())
        .then(new_guardian_review_id);
    let guardian_review =
        if let (Some(review_id), Some(request)) = (guardian_review_id.clone(), guardian_request) {
            Some(
                review_approval_request_with_review_detached(
                    session,
                    turn,
                    review_id,
                    request,
                    retry_reason.clone(),
                )
                .await,
            )
        } else {
            None
        };

    if let Some(mut request) = hook_request {
        request.guardian_review = guardian_review
            .as_ref()
            .map(|review| permission_request_guardian_review(review.review.clone()));
        match run_permission_request_hook_request(session, turn, request).await {
            Some(PermissionRequestDecision::Allow) => {
                return Some(PermissionApprovalOutcome::Decision {
                    decision: ReviewDecision::Approved,
                    source: PermissionApprovalSource::Hook,
                    guardian_review_id,
                });
            }
            Some(PermissionRequestDecision::Deny { message }) => {
                return Some(PermissionApprovalOutcome::HookDenied { message });
            }
            None => {}
        }
    }

    guardian_review.map(
        |GuardianApprovalReviewResult {
             decision,
             review: _,
         }| {
            PermissionApprovalOutcome::Decision {
                decision,
                source: PermissionApprovalSource::Guardian,
                guardian_review_id,
            }
        },
    )
}

fn permission_request_guardian_review(
    review: GuardianApprovalReview,
) -> PermissionRequestGuardianReview {
    PermissionRequestGuardianReview {
        status: match review.status {
            GuardianApprovalReviewStatus::Approved => HookGuardianReviewStatus::Approved,
            GuardianApprovalReviewStatus::Denied => HookGuardianReviewStatus::Denied,
            GuardianApprovalReviewStatus::Aborted => HookGuardianReviewStatus::Aborted,
            GuardianApprovalReviewStatus::Failed => HookGuardianReviewStatus::Failed,
            GuardianApprovalReviewStatus::TimedOut => HookGuardianReviewStatus::TimedOut,
        },
        decision: review.decision.map(|decision| match decision {
            GuardianAssessmentOutcome::Allow => PermissionRequestGuardianReviewDecision::Allow,
            GuardianAssessmentOutcome::Deny => PermissionRequestGuardianReviewDecision::Deny,
        }),
        risk_level: review.risk_level,
        user_authorization: review.user_authorization,
        rationale: review.rationale,
    }
}
