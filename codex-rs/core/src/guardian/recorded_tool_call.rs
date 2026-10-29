//! Prepares a supplied exec call and reviews it against captured history when approval is required.

use std::sync::Arc;

use codex_features::Feature;
use codex_guardian_reviewer::GuardianReviewError;
use codex_guardian_reviewer::ReviewDeadline;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_protocol::models::ResponseItem;
use codex_tools::FunctionCallError;
use codex_tools::ToolName;
use tokio_util::sync::CancellationToken;

use super::ApprovalRequestReasons;
use super::GuardianReviewContext;
use super::from_approval_action;
use super::review::ReviewInput;
use crate::context_manager::ContextManager;
use crate::guardian_review::GuardianReviewResult;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::tools::handlers::ToolEnvironmentArgs;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::resolve_tool_environment;
use crate::tools::handlers::unified_exec::preparation::ExecCommandLifetime;
use crate::tools::handlers::unified_exec::preparation::ParsedExecCommand;
use crate::tools::handlers::unified_exec::preparation::PreparedExecCommand;
use crate::tools::sandboxing::ExecApprovalRequirement;

pub(crate) async fn review_tool_call(
    session: &Arc<Session>,
    history: ContextManager,
    tool_call: &ResponseItem,
    cancellation: &CancellationToken,
) -> Result<GuardianReviewResult> {
    let call = RecordedExecCall::from_item(tool_call)?;
    if history.raw_items().any(|item| {
        matches!(item, ResponseItem::FunctionCall { name, namespace, .. }
            if name == "request_permissions"
                && ToolName::new(namespace.clone(), name).is_default_namespace())
    }) {
        return Err(CodexErr::InvalidRequest(
            "direct review does not support replaying request_permissions".into(),
        ));
    }
    let deadline = ReviewDeadline::from_timeout(session.get_config().await.guardian_review_timeout);

    let review = async {
        let turn = session
            .new_turn_with_default_settings(uuid::Uuid::now_v7().to_string(), Default::default())
            .await;
        let step = session
            .capture_speculative_step_context(turn, cancellation)
            .await?;
        let prepared = call.prepare(session, &step)?;
        let requirement = prepared.approval_requirement(session, &step).await;
        let approval = match &requirement {
            ExecApprovalRequirement::Skip { .. } => return Ok(None),
            ExecApprovalRequirement::NeedsApproval { reason, .. } => reason.clone(),
            ExecApprovalRequirement::NeedsHumanApproval { .. } => {
                return Err(CodexErr::InvalidRequest(
                    "Managed command policy requires human approval.".into(),
                ));
            }
            ExecApprovalRequirement::Forbidden { reason } => {
                return Err(CodexErr::InvalidRequest(reason.clone()));
            }
        };
        let action = prepared.approval_action(call.call_id, &requirement);
        let context = GuardianReviewContext::from(&step);
        let request = from_approval_action(action, &context)
            .request
            .map_err(CodexErr::InvalidRequest)?;
        let input = ReviewInput {
            session: Arc::clone(session),
            context,
            history,
            request,
            reasons: ApprovalRequestReasons {
                approval,
                retry: None,
            },
        };
        let assessment = input
            .assess_isolated(deadline, cancellation)
            .await
            .map_err(|error| match error {
                GuardianReviewError::Cancelled => CodexErr::TurnAborted,
                GuardianReviewError::Timeout => CodexErr::RequestTimeout,
                other => CodexErr::Fatal(format!("Guardian assessment failed: {other:?}")),
            })?;
        Ok(Some(assessment))
    };
    let assessment = tokio::select! {
        _ = cancellation.cancelled() => return Err(CodexErr::TurnAborted),
        _ = deadline.wait() => return Err(CodexErr::RequestTimeout),
        result = review => result?,
    };
    Ok(GuardianReviewResult {
        call_id: call.call_id.to_owned(),
        assessment,
    })
}

struct RecordedExecCall<'a> {
    call_id: &'a str,
    arguments: &'a str,
}

impl<'a> RecordedExecCall<'a> {
    fn from_item(item: &'a ResponseItem) -> Result<Self> {
        match item {
            ResponseItem::FunctionCall {
                call_id,
                name,
                namespace,
                arguments,
                status,
                encrypted_function_args,
                ..
            } if name == "exec_command"
                && encrypted_function_args.as_ref().is_none_or(Vec::is_empty)
                && ToolName::new(namespace.clone(), name).is_default_namespace()
                && !call_id.trim().is_empty()
                && matches!(status.as_deref(), None | Some("completed")) =>
            {
                Ok(Self { call_id, arguments })
            }
            _ => Err(CodexErr::InvalidRequest(
                "expected a completed plaintext exec_command call with a nonempty call ID".into(),
            )),
        }
    }

    fn prepare(&self, session: &Arc<Session>, step: &StepContext) -> Result<PreparedExecCommand> {
        if !step
            .tool_router
            .exposes_tool(&ToolName::plain("exec_command"))
        {
            return Err(CodexErr::InvalidRequest(
                "exec_command is unavailable on this thread".into(),
            ));
        }
        let args: ToolEnvironmentArgs = parse_arguments(self.arguments).map_err(tool_error)?;
        let environment =
            resolve_tool_environment(&step.environments, args.environment_id.as_deref())
                .map_err(tool_error)?;
        let parsed = ParsedExecCommand::parse(self.arguments, session, step, environment)
            .map_err(tool_error)?;
        let lifetime = if step.turn.config.features.enabled(Feature::UnifiedExec) {
            ExecCommandLifetime::Interactive
        } else {
            ExecCommandLifetime::OneShot
        };
        let prepared_exec = parsed.prepare(lifetime).map_err(tool_error)?;
        if codex_apply_patch::MaybeApplyPatch::intercepts_command(
            &prepared_exec.resolved.command,
            &prepared_exec.cwd,
        ) {
            return Err(CodexErr::InvalidRequest(
                "the recorded exec_command would be handled by apply_patch; direct review only supports ordinary commands".into(),
            ));
        }
        Ok(prepared_exec)
    }
}

fn tool_error(error: FunctionCallError) -> CodexErr {
    match error {
        FunctionCallError::RespondToModel(message) => CodexErr::InvalidRequest(message),
        FunctionCallError::Fatal(message) => CodexErr::Fatal(message),
    }
}
