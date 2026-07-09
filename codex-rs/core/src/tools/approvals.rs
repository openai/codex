//! Central approval policy-stage execution and reviewer routing.

use std::sync::Arc;

use crate::guardian::guardian_rejection_message;
use crate::guardian::guardian_timeout_message;
use crate::guardian::new_guardian_review_id;
use crate::guardian::review_approval_request;
use crate::guardian::routes_approval_to_guardian_with_reviewer;
use crate::hook_runtime::run_permission_request_hooks;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::flat_tool_name;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use codex_config::types::ApprovalsReviewer;
use codex_hooks::PermissionRequestDecision;
use codex_otel::ToolDecisionSource;
use codex_protocol::protocol::NetworkPolicyRuleAction;
use codex_protocol::protocol::ReviewDecision;

pub(super) type ApprovalAction = crate::guardian::GuardianApprovalRequest;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ApprovalReviewer {
    Guardian,
    User,
}

impl ApprovalReviewer {
    pub(super) fn for_turn(turn: &TurnContext) -> Self {
        Self::for_reviewer(turn, turn.config.approvals_reviewer)
    }

    fn for_reviewer(turn: &TurnContext, reviewer: ApprovalsReviewer) -> Self {
        if routes_approval_to_guardian_with_reviewer(turn, reviewer) {
            Self::Guardian
        } else {
            Self::User
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ApprovalResolutionSource {
    Hook,
    Guardian,
    User,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ApprovalResolution {
    decision: ReviewDecision,
    rejection: Option<String>,
    source: ApprovalResolutionSource,
}

impl ApprovalResolution {
    fn into_tool_result(self) -> Result<ReviewDecision, ToolError> {
        if let Some(rejection) = self.rejection {
            Err(ToolError::Rejected(rejection))
        } else {
            Ok(self.decision)
        }
    }
}

pub(super) async fn resolve_tool_apporval<Rq, Out, T>(
    tool: &mut T,
    req: &Rq,
    permission_request_run_id: &str,
    ctx: ApprovalCtx<'_>,
    tool_ctx: &ToolCtx,
    reviewer: ApprovalReviewer,
    otel: &codex_otel::SessionTelemetry,
) -> Result<ReviewDecision, ToolError>
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
                record_resolution(otel, tool_ctx, &resolution);
                return resolution.into_tool_result();
            }
            Some(PermissionRequestDecision::Deny { message }) => {
                let resolution = ApprovalResolution {
                    decision: ReviewDecision::Denied,
                    rejection: Some(message),
                    source: ApprovalResolutionSource::Hook,
                };
                record_resolution(otel, tool_ctx, &resolution);
                return resolution.into_tool_result();
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
                            "automatic approval review could not prepare the action".to_string(),
                        ),
                        source: ApprovalResolutionSource::Guardian,
                    };
                    record_resolution(otel, tool_ctx, &resolution);
                    return resolution.into_tool_result();
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
            normalize_guardian(ctx.session, review_id, decision).await
        }
        ApprovalReviewer::User => ApprovalResolution {
            decision: tool.start_approval_async(req, ctx.clone()).await,
            rejection: None,
            source: ApprovalResolutionSource::User,
        },
    };
    let resolution = normalize_user_rejection(resolution);
    record_resolution(otel, tool_ctx, &resolution);
    resolution.into_tool_result()
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
