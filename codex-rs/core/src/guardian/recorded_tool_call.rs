//! Prepares a supplied exec, patch, or permission request for review against captured history.
//! Preparation never executes the tool or grants permissions.

use std::sync::Arc;

use codex_features::Feature;
use codex_guardian_reviewer::GuardianReviewError;
use codex_guardian_reviewer::ReviewDeadline;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
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
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::PreparedPermissionRequest;
use crate::tools::handlers::ToolEnvironmentArgs;
use crate::tools::handlers::apply_patch::prepare_intercepted_patch;
use crate::tools::handlers::apply_patch::prepare_patch_request;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::resolve_tool_environment;
use crate::tools::handlers::selected_executor;
use crate::tools::handlers::unified_exec::preparation::ExecCommandLifetime;
use crate::tools::handlers::unified_exec::preparation::ParsedExecCommand;
use crate::tools::registry::PreToolUseError;
use crate::tools::registry::apply_pre_tool_use;
use crate::tools::sandboxing::ExecApprovalRequirement;

pub(crate) async fn review_tool_call(
    session: &Arc<Session>,
    history: ContextManager,
    tool_call: &ResponseItem,
    cancellation: &CancellationToken,
) -> Result<GuardianReviewResult> {
    match tool_call {
        ResponseItem::FunctionCall {
            call_id,
            name,
            namespace,
            status,
            encrypted_function_args,
            ..
        } if matches!(name.as_str(), "exec_command" | "request_permissions")
            && encrypted_function_args.as_ref().is_none_or(Vec::is_empty)
            && ToolName::new(namespace.clone(), name).is_default_namespace()
            && !call_id.trim().is_empty()
            && matches!(status.as_deref(), None | Some("completed")) => {}
        ResponseItem::CustomToolCall {
            call_id,
            name,
            namespace,
            status,
            ..
        } if name == "apply_patch"
            && ToolName::new(namespace.clone(), name).is_default_namespace()
            && !call_id.trim().is_empty()
            && matches!(status.as_deref(), None | Some("completed")) => {}
        _ => {
            return Err(CodexErr::InvalidRequest(
                "expected a completed plaintext exec_command, apply_patch, or request_permissions call with a nonempty call ID".into(),
            ));
        }
    };
    if history.raw_items().rev().skip(1).any(|item| {
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
        let parsed_call = step
            .tool_router
            .build_tool_call(tool_call.clone())
            .map_err(tool_error)?
            .ok_or_else(|| {
                CodexErr::InvalidRequest(
                    "expected an exec_command, apply_patch, or request_permissions call".into(),
                )
            })?;
        let tool = step
            .tool_router
            .registered_tool(&parsed_call.tool_name)
            .filter(|_| step.tool_router.exposes_tool(&parsed_call.tool_name))
            .ok_or_else(|| {
                CodexErr::InvalidRequest("the requested tool is unavailable on this thread".into())
            })?;
        let source = parsed_call.direct_source();
        let mut invocation = ToolInvocation {
            session: Arc::clone(session),
            turn: Arc::clone(&step.turn),
            step_context: Arc::clone(&step),
            cancellation_token: cancellation.clone(),
            tracker: Default::default(),
            call_id: parsed_call.call_id,
            tool_name: parsed_call.tool_name,
            source,
            payload: parsed_call.payload,
            native_call: parsed_call.native_call,
            environments: Default::default(),
        };
        invocation.environments = tool
            .runtime
            .environments_for_input(&invocation)
            .map_err(tool_error)?;
        apply_pre_tool_use(tool.runtime.as_ref(), &mut invocation)
            .await
            .map_err(|err| match err {
                PreToolUseError::Blocked(reason) => CodexErr::InvalidRequest(reason),
                PreToolUseError::Failed(err) => tool_error(err),
                PreToolUseError::Cancelled(_) => CodexErr::TurnAborted,
            })?;
        let step = &invocation.step_context;
        let (requirement, action) = match (invocation.tool_name.name.as_str(), &invocation.payload)
        {
            ("exec_command", ToolPayload::Function { arguments }) => {
                let args: ToolEnvironmentArgs = parse_arguments(arguments).map_err(tool_error)?;
                let environment =
                    resolve_tool_environment(&step.environments, args.environment_id.as_deref())
                        .map_err(tool_error)?;
                let parsed = ParsedExecCommand::parse(arguments, session, step, environment)
                    .map_err(tool_error)?;
                let lifetime = if step.turn.config.features.enabled(Feature::UnifiedExec) {
                    ExecCommandLifetime::Interactive
                } else {
                    ExecCommandLifetime::OneShot
                };
                let prepared = parsed.prepare(lifetime).map_err(tool_error)?;
                let fs = prepared.turn_environment.environment().get_filesystem();
                if let Some(patch) = prepare_intercepted_patch(
                    &prepared.resolved.command,
                    &prepared.cwd,
                    fs.as_ref(),
                    &prepared.turn_environment,
                    step,
                )
                .await
                .map_err(tool_error)?
                {
                    let action = patch.approval_action(&invocation.call_id);
                    (Some(patch.exec_approval_requirement), action)
                } else {
                    let requirement = prepared.approval_requirement(session, step).await;
                    let action = prepared.approval_action(&invocation.call_id, &requirement);
                    (Some(requirement), action)
                }
            }
            ("apply_patch", ToolPayload::Custom { input }) => {
                let environment = selected_executor(&invocation.environments, &step.environments)
                    .map_err(tool_error)?;
                let prepared = prepare_patch_request(input, environment, step)
                    .await
                    .map_err(tool_error)?;
                let action = prepared.approval_action(&invocation.call_id);
                (Some(prepared.exec_approval_requirement), action)
            }
            ("request_permissions", ToolPayload::Function { arguments }) => {
                let environment = selected_executor(&invocation.environments, &step.environments)
                    .map_err(tool_error)?;
                let prepared =
                    PreparedPermissionRequest::parse(arguments, environment).map_err(tool_error)?;
                match step.settings.approval_policy() {
                    AskForApproval::Never => {
                        return Ok(GuardianReviewResult {
                            call_id: invocation.call_id,
                            assessment: None,
                        });
                    }
                    AskForApproval::Granular(policy) if !policy.allows_request_permissions() => {
                        return Ok(GuardianReviewResult {
                            call_id: invocation.call_id,
                            assessment: None,
                        });
                    }
                    AskForApproval::OnRequest
                    | AskForApproval::UnlessTrusted
                    | AskForApproval::Granular(_) => {}
                }
                let action = prepared.approval_action(&invocation.call_id, &step.turn.sub_id);
                (None, action)
            }
            _ => {
                return Err(CodexErr::Fatal(
                    "unsupported tool or payload for direct review".into(),
                ));
            }
        };
        let approval = match &requirement {
            None => None,
            Some(ExecApprovalRequirement::Skip { .. }) => {
                return Ok(GuardianReviewResult {
                    call_id: invocation.call_id,
                    assessment: None,
                });
            }
            Some(ExecApprovalRequirement::NeedsApproval { reason, .. }) => reason.clone(),
            Some(ExecApprovalRequirement::NeedsHumanApproval { .. }) => {
                return Err(CodexErr::InvalidRequest(
                    "Managed command policy requires human approval.".into(),
                ));
            }
            Some(ExecApprovalRequirement::Forbidden { reason }) => {
                return Err(CodexErr::InvalidRequest(reason.clone()));
            }
        };
        let context = GuardianReviewContext::from(step);
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
        Ok(GuardianReviewResult {
            call_id: invocation.call_id,
            assessment: Some(assessment),
        })
    };
    tokio::select! {
        _ = cancellation.cancelled() => Err(CodexErr::TurnAborted),
        _ = deadline.wait() => Err(CodexErr::RequestTimeout),
        result = review => result,
    }
}

fn tool_error(error: FunctionCallError) -> CodexErr {
    match error {
        FunctionCallError::RespondToModel(message) => CodexErr::InvalidRequest(message),
        FunctionCallError::Fatal(message) => CodexErr::Fatal(message),
    }
}
