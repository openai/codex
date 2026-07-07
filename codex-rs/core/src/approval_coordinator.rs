//! Central approval policy-stage execution and reviewer routing.

use std::future::Future;
use std::sync::Arc;

use crate::guardian::guardian_rejection_message;
use crate::guardian::guardian_timeout_message;
use crate::guardian::new_guardian_review_id;
use crate::guardian::review_approval_request;
use crate::hook_runtime::run_permission_request_hooks;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::flat_tool_name;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::PermissionRequestPayload;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use codex_config::types::ApprovalsReviewer;
use codex_hooks::PermissionRequestDecision;
use codex_otel::ToolDecisionSource;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::NetworkPolicyRuleAction;
use codex_protocol::protocol::ReviewDecision;

pub(crate) type ApprovalAction = crate::guardian::GuardianApprovalRequest;

#[derive(Clone, Debug)]
pub(crate) struct ApprovalReview {
    pub(crate) action: ApprovalAction,
    pub(crate) retry_reason: Option<String>,
}

impl ApprovalReview {
    pub(crate) fn main_turn(action: ApprovalAction, retry_reason: Option<String>) -> Self {
        Self {
            action,
            retry_reason,
        }
    }
}

pub(crate) struct ApprovalHookRequest<'a> {
    pub(crate) run_id: &'a str,
    pub(crate) payload: PermissionRequestPayload,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ApprovalReviewer {
    Guardian,
    User,
}

impl ApprovalReviewer {
    pub(crate) fn for_turn(turn: &TurnContext) -> Self {
        Self::for_reviewer(turn, turn.config.approvals_reviewer)
    }

    pub(crate) fn for_reviewer(turn: &TurnContext, reviewer: ApprovalsReviewer) -> Self {
        if Self::routes_to_guardian(turn, reviewer) {
            Self::Guardian
        } else {
            Self::User
        }
    }

    pub(crate) fn automatic_for_reviewer(
        turn: &TurnContext,
        reviewer: ApprovalsReviewer,
    ) -> Option<Self> {
        Self::routes_to_guardian(turn, reviewer).then_some(Self::Guardian)
    }

    fn routes_to_guardian(turn: &TurnContext, reviewer: ApprovalsReviewer) -> bool {
        matches!(
            turn.approval_policy.value(),
            AskForApproval::OnRequest | AskForApproval::Granular(_)
        ) && reviewer == ApprovalsReviewer::AutoReview
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ApprovalResolutionSource {
    Hook,
    Guardian,
    User,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ApprovalResolution {
    pub(crate) decision: ReviewDecision,
    pub(crate) rejection: Option<String>,
    pub(crate) source: ApprovalResolutionSource,
}

pub(crate) struct ApprovalUserReview<T> {
    pub(crate) decision: ReviewDecision,
    pub(crate) output: T,
}

pub(crate) struct ApprovalEventResolution<T> {
    pub(crate) resolution: ApprovalResolution,
    pub(crate) user_output: Option<T>,
}

impl ApprovalResolution {
    pub(crate) fn into_tool_result(self) -> Result<ReviewDecision, ToolError> {
        if let Some(rejection) = self.rejection {
            Err(ToolError::Rejected(rejection))
        } else {
            Ok(self.decision)
        }
    }
}

pub(crate) struct ApprovalCoordinator;

impl ApprovalCoordinator {
    pub(crate) async fn resolve_tool<Rq, Out, T>(
        tool: &mut T,
        req: &Rq,
        permission_request_run_id: &str,
        ctx: ApprovalCtx<'_>,
        tool_ctx: &ToolCtx,
        reviewer: ApprovalReviewer,
        otel: &codex_otel::SessionTelemetry,
    ) -> Result<ApprovalResolution, ToolError>
    where
        T: ToolRuntime<Rq, Out>,
    {
        if let Some(permission_request) = tool.permission_request_payload(req) {
            match run_permission_request_hooks(
                ctx.session,
                ctx.turn,
                permission_request_run_id,
                permission_request,
            )
            .await
            {
                Some(PermissionRequestDecision::Allow) => {
                    let resolution = ApprovalResolution {
                        decision: ReviewDecision::Approved,
                        rejection: None,
                        source: ApprovalResolutionSource::Hook,
                    };
                    Self::record_resolution(otel, tool_ctx, &resolution);
                    return Ok(resolution);
                }
                Some(PermissionRequestDecision::Deny { message }) => {
                    let resolution = ApprovalResolution {
                        decision: ReviewDecision::Denied,
                        rejection: Some(message),
                        source: ApprovalResolutionSource::Hook,
                    };
                    Self::record_resolution(otel, tool_ctx, &resolution);
                    return Ok(resolution);
                }
                None => {}
            }
        }

        let resolution = match reviewer {
            ApprovalReviewer::Guardian => {
                let review_id = new_guardian_review_id();
                let action = match tool.approval_action(req, &ctx) {
                    Ok(action) => action,
                    Err(err) => {
                        tracing::error!(%err, "failed to build automatic approval action");
                        let resolution = ApprovalResolution {
                            decision: ReviewDecision::Abort,
                            rejection: Some(
                                "automatic approval review could not prepare the action"
                                    .to_string(),
                            ),
                            source: ApprovalResolutionSource::Guardian,
                        };
                        Self::record_resolution(otel, tool_ctx, &resolution);
                        return Ok(resolution);
                    }
                };
                let decision = review_approval_request(
                    ctx.session,
                    ctx.turn,
                    review_id.clone(),
                    action,
                    ctx.retry_reason.clone(),
                )
                .await;
                Self::normalize_guardian(ctx.session, review_id, decision).await
            }
            ApprovalReviewer::User => ApprovalResolution {
                decision: tool.start_approval_async(req, ctx.clone()).await,
                rejection: None,
                source: ApprovalResolutionSource::User,
            },
        };
        let resolution = Self::normalize_user_rejection(resolution);
        Self::record_resolution(otel, tool_ctx, &resolution);
        Ok(resolution)
    }

    pub(crate) async fn resolve_event<F, Fut>(
        session: &Arc<Session>,
        turn: &Arc<TurnContext>,
        reviewer: ApprovalReviewer,
        hook_request: Option<ApprovalHookRequest<'_>>,
        review: ApprovalReview,
        user_review: F,
    ) -> ApprovalResolution
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ReviewDecision>,
    {
        Self::resolve_event_with_user_output(
            session,
            turn,
            reviewer,
            hook_request,
            review,
            || async {
            ApprovalUserReview {
                decision: user_review().await,
                output: (),
            }
        },
        )
        .await
        .resolution
    }

    pub(crate) async fn resolve_automatic_event(
        session: &Arc<Session>,
        turn: &Arc<TurnContext>,
        reviewer: ApprovalReviewer,
        hook_request: Option<ApprovalHookRequest<'_>>,
        review: ApprovalReview,
    ) -> ApprovalResolution {
        debug_assert_eq!(reviewer, ApprovalReviewer::Guardian);
        Self::resolve_event_with_user_output(session, turn, reviewer, hook_request, review, || async {
            ApprovalUserReview {
                decision: ReviewDecision::Denied,
                output: (),
            }
        })
        .await
        .resolution
    }

    pub(crate) async fn resolve_event_with_user_output<T, F, Fut>(
        session: &Arc<Session>,
        turn: &Arc<TurnContext>,
        reviewer: ApprovalReviewer,
        hook_request: Option<ApprovalHookRequest<'_>>,
        review: ApprovalReview,
        user_review: F,
    ) -> ApprovalEventResolution<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ApprovalUserReview<T>>,
    {
        if let Some(hook_request) = hook_request
            && let Some(resolution) =
                Self::resolve_hook(session, turn, hook_request.run_id, hook_request.payload).await
        {
            return ApprovalEventResolution {
                resolution,
                user_output: None,
            };
        }

        let (resolution, user_output) = match reviewer {
            ApprovalReviewer::Guardian => {
                let review_id = new_guardian_review_id();
                let decision = review_approval_request(
                    session,
                    turn,
                    review_id.clone(),
                    review.action.clone(),
                    review.retry_reason.clone(),
                )
                .await;
                (
                    Self::normalize_guardian(session, review_id, decision).await,
                    None,
                )
            }
            ApprovalReviewer::User => {
                let user_review = user_review().await;
                (
                    ApprovalResolution {
                        decision: user_review.decision,
                        rejection: None,
                        source: ApprovalResolutionSource::User,
                    },
                    Some(user_review.output),
                )
            }
        };
        ApprovalEventResolution {
            resolution: Self::normalize_user_rejection(resolution),
            user_output,
        }
    }

    async fn resolve_hook(
        session: &Arc<Session>,
        turn: &Arc<TurnContext>,
        run_id: &str,
        payload: PermissionRequestPayload,
    ) -> Option<ApprovalResolution> {
        match run_permission_request_hooks(session, turn, run_id, payload).await {
            Some(PermissionRequestDecision::Allow) => Some(ApprovalResolution {
                decision: ReviewDecision::Approved,
                rejection: None,
                source: ApprovalResolutionSource::Hook,
            }),
            Some(PermissionRequestDecision::Deny { message }) => Some(ApprovalResolution {
                decision: ReviewDecision::Denied,
                rejection: Some(message),
                source: ApprovalResolutionSource::Hook,
            }),
            None => None,
        }
    }

    async fn normalize_guardian(
        session: &Arc<Session>,
        review_id: String,
        decision: ReviewDecision,
    ) -> ApprovalResolution {
        let rejection = match &decision {
            ReviewDecision::Approved
            | ReviewDecision::ApprovedForSession
            | ReviewDecision::ApprovedExecpolicyAmendment { .. } => None,
            ReviewDecision::NetworkPolicyAmendment {
                network_policy_amendment,
            } if network_policy_amendment.action == NetworkPolicyRuleAction::Allow => None,
            ReviewDecision::TimedOut => Some(guardian_timeout_message()),
            ReviewDecision::NetworkPolicyAmendment { .. }
            | ReviewDecision::Denied
            | ReviewDecision::Abort => {
                Some(guardian_rejection_message(session.as_ref(), &review_id).await)
            }
        };
        ApprovalResolution {
            decision,
            rejection,
            source: ApprovalResolutionSource::Guardian,
        }
    }

    fn normalize_user_rejection(mut resolution: ApprovalResolution) -> ApprovalResolution {
        if resolution.source == ApprovalResolutionSource::User {
            resolution.rejection = match &resolution.decision {
                ReviewDecision::Approved
                | ReviewDecision::ApprovedForSession
                | ReviewDecision::ApprovedExecpolicyAmendment { .. } => None,
                ReviewDecision::NetworkPolicyAmendment {
                    network_policy_amendment,
                } if network_policy_amendment.action == NetworkPolicyRuleAction::Allow => None,
                ReviewDecision::NetworkPolicyAmendment { .. }
                | ReviewDecision::Denied
                | ReviewDecision::Abort => Some("rejected by user".to_string()),
                ReviewDecision::TimedOut => Some("approval request timed out".to_string()),
            };
        }
        resolution
    }

    fn record_resolution(
        otel: &codex_otel::SessionTelemetry,
        tool_ctx: &ToolCtx,
        resolution: &ApprovalResolution,
    ) {
        let source = match resolution.source {
            ApprovalResolutionSource::Hook => ToolDecisionSource::Config,
            ApprovalResolutionSource::Guardian => ToolDecisionSource::AutomatedReviewer,
            ApprovalResolutionSource::User => ToolDecisionSource::User,
        };
        let tool_name = flat_tool_name(&tool_ctx.tool_name);
        otel.tool_decision(
            tool_name.as_ref(),
            &tool_ctx.call_id,
            &resolution.decision,
            source,
        );
    }
}
