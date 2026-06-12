//! Ordered approval dispatch for the normal orchestrated tool runtimes.

use crate::guardian::format_guardian_action_pretty;
use crate::guardian::guardian_assessment_action;
use crate::guardian::guardian_request_target_item_id;
use crate::guardian::guardian_request_turn_id;
use crate::guardian::review_approval_request;
use crate::hook_runtime::run_permission_request_hooks;
use crate::tools::flat_tool_name;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use codex_extension_api::ApprovalReviewInput;
use codex_extension_api::ApprovalReviewOutcome;
use codex_extension_api::ApprovalReviewSource;
use codex_hooks::PermissionRequestDecision;
use codex_otel::ToolDecisionSource;
use codex_protocol::protocol::ReviewDecision;

pub(crate) async fn request_approval<Rq, Out, T>(
    tool: &mut T,
    req: &Rq,
    permission_request_run_id: &str,
    approval_ctx: ApprovalCtx<'_>,
    tool_ctx: &ToolCtx,
    evaluate_permission_request_hooks: bool,
    otel: &codex_otel::SessionTelemetry,
) -> Result<ReviewDecision, ToolError>
where
    T: ToolRuntime<Rq, Out>,
{
    let tool_name = flat_tool_name(&tool_ctx.tool_name);
    if evaluate_permission_request_hooks
        && let Some(permission_request) = tool.permission_request_payload(req)
    {
        match run_permission_request_hooks(
            approval_ctx.session,
            approval_ctx.turn,
            permission_request_run_id,
            permission_request,
        )
        .await
        {
            Some(PermissionRequestDecision::Allow) => {
                let decision = ReviewDecision::Approved;
                otel.tool_decision(
                    tool_name.as_ref(),
                    &tool_ctx.call_id,
                    &decision,
                    ToolDecisionSource::Config,
                );
                return Ok(decision);
            }
            Some(PermissionRequestDecision::Deny { message }) => {
                let decision = ReviewDecision::Denied;
                otel.tool_decision(
                    tool_name.as_ref(),
                    &tool_ctx.call_id,
                    &decision,
                    ToolDecisionSource::Config,
                );
                return Err(ToolError::Rejected(message));
            }
            None => {}
        }
    }

    if let Some(review_id) = approval_ctx.guardian_review_id.clone() {
        let request = tool
            .guardian_approval_request(req, approval_ctx.call_id)
            .ok_or_else(|| {
                ToolError::Rejected(
                    "automatic approval review is not supported for this tool".to_string(),
                )
            })?;
        let action = guardian_assessment_action(&request);
        let prompt = format_guardian_action_pretty(&request)
            .map_err(|error| {
                ToolError::Rejected(format!("failed to render review prompt: {error}"))
            })?
            .text;
        let approval_policy = approval_ctx.turn.approval_policy.value();
        let review_input = ApprovalReviewInput {
            session_store: &approval_ctx.session.services.session_extension_data,
            thread_store: &approval_ctx.session.services.thread_extension_data,
            turn_store: approval_ctx.turn.extension_data.as_ref(),
            review_id: &review_id,
            turn_id: guardian_request_turn_id(&request, &approval_ctx.turn.sub_id),
            target_item_id: guardian_request_target_item_id(&request),
            prompt: &prompt,
            action: &action,
            reviewer: approval_ctx.turn.config.approvals_reviewer,
            approval_policy: &approval_policy,
            retry_reason: approval_ctx.retry_reason.as_deref(),
            source: ApprovalReviewSource::MainTurn,
        };

        let decision = match approval_ctx
            .session
            .services
            .extensions
            .approval_review(review_input)
            .await
        {
            Ok(ApprovalReviewOutcome::Decision {
                decision,
                denial_message,
            }) => {
                if matches!(decision, ReviewDecision::Denied | ReviewDecision::Abort) {
                    let message = denial_message.unwrap_or_else(|| {
                        "automatic approval reviewer denied the action".to_string()
                    });
                    otel.tool_decision(
                        tool_name.as_ref(),
                        &tool_ctx.call_id,
                        &decision,
                        ToolDecisionSource::AutomatedReviewer,
                    );
                    return Err(ToolError::Rejected(message));
                }
                decision
            }
            Ok(ApprovalReviewOutcome::Abstain) => {
                review_approval_request(
                    approval_ctx.session,
                    approval_ctx.turn,
                    review_id,
                    request,
                    approval_ctx.retry_reason,
                )
                .await
            }
            Err(error) => {
                let decision = ReviewDecision::Denied;
                otel.tool_decision(
                    tool_name.as_ref(),
                    &tool_ctx.call_id,
                    &decision,
                    ToolDecisionSource::AutomatedReviewer,
                );
                return Err(ToolError::Rejected(format!(
                    "automatic approval review failed: {error}"
                )));
            }
        };
        otel.tool_decision(
            tool_name.as_ref(),
            &tool_ctx.call_id,
            &decision,
            ToolDecisionSource::AutomatedReviewer,
        );
        return Ok(decision);
    }

    let decision = tool.start_approval_async(req, approval_ctx).await;
    otel.tool_decision(
        tool_name.as_ref(),
        &tool_ctx.call_id,
        &decision,
        ToolDecisionSource::User,
    );
    Ok(decision)
}
