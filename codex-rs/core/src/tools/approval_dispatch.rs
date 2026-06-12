//! Ordered approval dispatch for the normal orchestrated tool runtimes.

use crate::guardian::GuardianApprovalRequest;
use crate::guardian::format_guardian_action_pretty;
use crate::guardian::guardian_assessment_action;
use crate::guardian::guardian_rejection_message;
use crate::guardian::guardian_request_target_item_id;
use crate::guardian::guardian_request_turn_id;
use crate::guardian::guardian_timeout_message;
use crate::guardian::review_approval_request;
use crate::hook_runtime::run_permission_request_hooks;
use crate::tools::flat_tool_name;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use codex_extension_api::ApprovalReviewInput;
use codex_extension_api::ApprovalReviewOutcome;
use codex_extension_api::ApprovalReviewRunner;
use codex_extension_api::ApprovalReviewSource;
use codex_extension_api::ExtensionFuture;
use codex_hooks::PermissionRequestDecision;
use codex_otel::ToolDecisionSource;
use codex_protocol::protocol::ReviewDecision;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AutomatedApprovalSource {
    Extension,
    ExtensionError,
    Guardian,
}

#[derive(Debug)]
pub(crate) struct AutomatedApprovalDecision {
    pub decision: ReviewDecision,
    pub denial_message: Option<String>,
    pub source: AutomatedApprovalSource,
}

impl AutomatedApprovalDecision {
    pub(crate) fn denial_message(&self) -> String {
        self.denial_message.clone().unwrap_or_else(|| {
            if self.decision == ReviewDecision::TimedOut {
                return guardian_timeout_message();
            }
            match self.source {
                AutomatedApprovalSource::Extension => {
                    "automatic approval reviewer denied the action".to_string()
                }
                AutomatedApprovalSource::ExtensionError => {
                    "automatic approval review failed".to_string()
                }
                AutomatedApprovalSource::Guardian => "Guardian denied this request.".to_string(),
            }
        })
    }
}

struct CoreApprovalReviewRunner<'a> {
    session: &'a std::sync::Arc<crate::session::session::Session>,
    turn: &'a std::sync::Arc<crate::session::turn_context::TurnContext>,
    review_id: &'a str,
    request: &'a GuardianApprovalRequest,
    retry_reason: Option<&'a str>,
    source: ApprovalReviewSource,
    cancellation_token: Option<&'a CancellationToken>,
}

impl ApprovalReviewRunner for CoreApprovalReviewRunner<'_> {
    fn run(
        &self,
    ) -> ExtensionFuture<'_, Result<ApprovalReviewOutcome, codex_extension_api::ApprovalReviewError>>
    {
        Box::pin(async move {
            let decision = if let Some(cancellation_token) = self.cancellation_token {
                let review_rx = crate::guardian::spawn_approval_request_review(
                    std::sync::Arc::clone(self.session),
                    std::sync::Arc::clone(self.turn),
                    self.review_id.to_string(),
                    self.request.clone(),
                    self.retry_reason.map(str::to_string),
                    match self.source {
                        ApprovalReviewSource::MainTurn => {
                            codex_analytics::GuardianApprovalRequestSource::MainTurn
                        }
                        ApprovalReviewSource::DelegatedSubagent => {
                            codex_analytics::GuardianApprovalRequestSource::DelegatedSubagent
                        }
                    },
                    cancellation_token.clone(),
                );
                review_rx.await.unwrap_or(ReviewDecision::Denied)
            } else {
                review_approval_request(
                    self.session,
                    self.turn,
                    self.review_id.to_string(),
                    self.request.clone(),
                    self.retry_reason.map(str::to_string),
                )
                .await
            };
            let denial_message = match decision {
                ReviewDecision::Denied | ReviewDecision::Abort => {
                    Some(guardian_rejection_message(self.session, self.review_id).await)
                }
                ReviewDecision::TimedOut => Some(guardian_timeout_message()),
                _ => None,
            };
            Ok(ApprovalReviewOutcome::Decision {
                decision,
                denial_message,
            })
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn request_automated_approval(
    session: &std::sync::Arc<crate::session::session::Session>,
    turn: &std::sync::Arc<crate::session::turn_context::TurnContext>,
    review_id: String,
    request: GuardianApprovalRequest,
    reviewer: codex_protocol::config_types::ApprovalsReviewer,
    retry_reason: Option<String>,
    source: ApprovalReviewSource,
) -> Result<AutomatedApprovalDecision, String> {
    let action = guardian_assessment_action(&request);
    let prompt = format_guardian_action_pretty(&request)
        .map_err(|error| format!("failed to render review prompt: {error}"))?
        .text;
    let approval_policy = turn.approval_policy.value();
    let runner = CoreApprovalReviewRunner {
        session,
        turn,
        review_id: &review_id,
        request: &request,
        retry_reason: retry_reason.as_deref(),
        source,
        cancellation_token: None,
    };
    let review_input = ApprovalReviewInput {
        session_store: &session.services.session_extension_data,
        thread_store: &session.services.thread_extension_data,
        turn_store: turn.extension_data.as_ref(),
        review_id: &review_id,
        turn_id: guardian_request_turn_id(&request, &turn.sub_id),
        target_item_id: guardian_request_target_item_id(&request),
        request: &request,
        prompt: &prompt,
        action: &action,
        reviewer,
        approval_policy: &approval_policy,
        retry_reason: retry_reason.as_deref(),
        source,
        runner: &runner,
    };
    match session
        .services
        .extensions
        .approval_review(review_input)
        .await
    {
        Ok(ApprovalReviewOutcome::Decision {
            decision,
            denial_message,
        }) => Ok(AutomatedApprovalDecision {
            decision,
            denial_message,
            source: AutomatedApprovalSource::Extension,
        }),
        Ok(ApprovalReviewOutcome::Abstain) => {
            let ApprovalReviewOutcome::Decision {
                decision,
                denial_message,
            } = runner.run().await.map_err(|error| error.to_string())?
            else {
                return Err("core approval reviewer unexpectedly abstained".to_string());
            };
            Ok(AutomatedApprovalDecision {
                decision,
                denial_message,
                source: AutomatedApprovalSource::Guardian,
            })
        }
        Err(error) => Err(format!("automatic approval review failed: {error}")),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn request_automated_approval_with_cancel(
    session: &std::sync::Arc<crate::session::session::Session>,
    turn: &std::sync::Arc<crate::session::turn_context::TurnContext>,
    review_id: String,
    request: GuardianApprovalRequest,
    reviewer: codex_protocol::config_types::ApprovalsReviewer,
    retry_reason: Option<String>,
    source: ApprovalReviewSource,
    cancellation_token: CancellationToken,
) -> Option<Result<AutomatedApprovalDecision, String>> {
    let action = guardian_assessment_action(&request);
    let prompt = match format_guardian_action_pretty(&request) {
        Ok(prompt) => prompt.text,
        Err(error) => {
            return Some(Err(format!("failed to render review prompt: {error}")));
        }
    };
    let approval_policy = turn.approval_policy.value();
    let runner = CoreApprovalReviewRunner {
        session,
        turn,
        review_id: &review_id,
        request: &request,
        retry_reason: retry_reason.as_deref(),
        source,
        cancellation_token: Some(&cancellation_token),
    };
    let review_input = ApprovalReviewInput {
        session_store: &session.services.session_extension_data,
        thread_store: &session.services.thread_extension_data,
        turn_store: turn.extension_data.as_ref(),
        review_id: &review_id,
        turn_id: guardian_request_turn_id(&request, &turn.sub_id),
        target_item_id: guardian_request_target_item_id(&request),
        request: &request,
        prompt: &prompt,
        action: &action,
        reviewer,
        approval_policy: &approval_policy,
        retry_reason: retry_reason.as_deref(),
        source,
        runner: &runner,
    };

    let extension_outcome = tokio::select! {
        biased;
        _ = cancellation_token.cancelled() => return None,
        outcome = session.services.extensions.approval_review(review_input) => outcome,
    };
    match extension_outcome {
        Ok(ApprovalReviewOutcome::Decision {
            decision,
            denial_message,
        }) => Some(Ok(AutomatedApprovalDecision {
            decision,
            denial_message,
            source: AutomatedApprovalSource::Extension,
        })),
        Ok(ApprovalReviewOutcome::Abstain) => {
            let outcome = tokio::select! {
                biased;
                _ = cancellation_token.cancelled() => return None,
                outcome = runner.run() => outcome,
            };
            let outcome = match outcome {
                Ok(outcome) => outcome,
                Err(error) => return Some(Err(error.to_string())),
            };
            let ApprovalReviewOutcome::Decision {
                decision,
                denial_message,
            } = outcome
            else {
                return Some(Err(
                    "core approval reviewer unexpectedly abstained".to_string()
                ));
            };
            Some(Ok(AutomatedApprovalDecision {
                decision,
                denial_message,
                source: AutomatedApprovalSource::Guardian,
            }))
        }
        Err(error) => Some(Err(format!("automatic approval review failed: {error}"))),
    }
}

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
        let automated = match request_automated_approval(
            approval_ctx.session,
            approval_ctx.turn,
            review_id,
            request,
            approval_ctx.turn.config.approvals_reviewer,
            approval_ctx.retry_reason,
            ApprovalReviewSource::MainTurn,
        )
        .await
        {
            Ok(automated) => automated,
            Err(message) => {
                otel.tool_decision(
                    tool_name.as_ref(),
                    &tool_ctx.call_id,
                    &ReviewDecision::Denied,
                    ToolDecisionSource::AutomatedReviewer,
                );
                return Err(ToolError::Rejected(message));
            }
        };
        let decision = automated.decision.clone();
        otel.tool_decision(
            tool_name.as_ref(),
            &tool_ctx.call_id,
            &decision,
            ToolDecisionSource::AutomatedReviewer,
        );
        if matches!(decision, ReviewDecision::Denied | ReviewDecision::Abort) {
            return Err(ToolError::Rejected(automated.denial_message()));
        }
        if decision == ReviewDecision::TimedOut {
            return Err(ToolError::Rejected(
                automated
                    .denial_message
                    .unwrap_or_else(guardian_timeout_message),
            ));
        }
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
