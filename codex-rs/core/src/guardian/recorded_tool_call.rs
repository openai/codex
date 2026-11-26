//! Prepares a recorded tool call for review against captured history and current policy.
//! Preparation never executes the tool or grants permissions.

use std::sync::Arc;

use codex_analytics::GuardianApprovalRequestSource;
use codex_app_server_protocol::GuardianRootEvidence;
use codex_app_server_protocol::GuardianRootEvidenceMessage;
use codex_features::Feature;
use codex_guardian_context::GuardianRootMessage;
use codex_guardian_reviewer::GuardianAssessment;
use codex_guardian_reviewer::GuardianReviewError;
use codex_guardian_reviewer::GuardianReviewOutcome;
use codex_guardian_reviewer::ReviewDeadline;
use codex_guardian_reviewer::ReviewHost;
use codex_protocol::approvals::GuardianReviewReason;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ReviewDecision;
use codex_tools::FunctionCallError;
use codex_tools::ToolName;
use tokio_util::sync::CancellationToken;

use super::ApprovalRequestReasons;
use super::GuardianReviewContext;
use super::GuardianReviewOptions;
use super::from_approval_action;
use super::new_guardian_review_id;
use super::review::PreparedApproval;
use super::review::ReviewInput;
use super::runtime::ReviewRuntime;
use crate::context_manager::ContextManager;
use crate::guardian_review::AttachedReviewer;
use crate::guardian_review::GuardianReviewResult;
use crate::guardian_review::RecordedReviewMode;
use crate::mcp_tool_call;
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
    mode: RecordedReviewMode,
    cancellation: &CancellationToken,
) -> Result<GuardianReviewResult> {
    let is_supported_builtin = match tool_call {
        ResponseItem::FunctionCall {
            call_id,
            name,
            namespace,
            status,
            encrypted_function_args,
            ..
        } if encrypted_function_args.as_ref().is_none_or(Vec::is_empty)
            && !call_id.trim().is_empty()
            && matches!(status.as_deref(), None | Some("completed")) =>
        {
            matches!(name.as_str(), "exec_command" | "request_permissions")
                && ToolName::new(namespace.clone(), name).is_default_namespace()
        }
        ResponseItem::CustomToolCall {
            call_id,
            name,
            namespace,
            status,
            ..
        } if name == "apply_patch"
            && ToolName::new(namespace.clone(), name).is_default_namespace()
            && !call_id.trim().is_empty()
            && matches!(status.as_deref(), None | Some("completed")) =>
        {
            true
        }
        _ => {
            return Err(CodexErr::InvalidRequest(
                "expected a completed plaintext exec_command, apply_patch, request_permissions, or MCP call with a nonempty call ID".into(),
            ));
        }
    };
    if history.raw_items().any(|item| {
        matches!(item, ResponseItem::FunctionCall { name, namespace, .. }
            if name == "request_permissions"
                && ToolName::new(namespace.clone(), name).is_default_namespace())
    }) {
        return Err(CodexErr::InvalidRequest(
            "direct review does not support replaying request_permissions".into(),
        ));
    }
    if let Some(evidence) = mode.root_evidence()
        && !evidence.messages.iter().any(
            |entry| matches!(entry, GuardianRootEvidenceMessage::User(text) if !text.trim().is_empty()),
        )
    {
        return Err(CodexErr::InvalidRequest("missing root user evidence".into()));
    }
    let deadline = ReviewDeadline::from_timeout(session.get_config().await.guardian_review_timeout);

    let resolve_recorded_call = async |root_evidence: Option<GuardianRootEvidence>| {
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
                    "expected an exec_command, apply_patch, request_permissions, or MCP call"
                        .into(),
                )
            })?;
        let tool = step
            .tool_router
            .registered_tool(&parsed_call.tool_name)
            .filter(|_| step.tool_router.exposes_tool(&parsed_call.tool_name))
            .ok_or_else(|| {
                CodexErr::InvalidRequest("the requested tool is unavailable on this thread".into())
            })?;
        let advertised_mcp_tool = tool.runtime.mcp_tool_info();
        if !is_supported_builtin && advertised_mcp_tool.is_none() {
            return Err(CodexErr::InvalidRequest(
                "unsupported tool for direct review".into(),
            ));
        }
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
        let context = GuardianReviewContext {
            root_evidence: root_evidence.map(Arc::new),
            ..GuardianReviewContext::from(step)
        };
        let (approval, request) = if let Some(tool_info) = advertised_mcp_tool {
            let request = mcp_tool_call::recorded::prepare_review_request(&invocation, tool_info)
                .await
                .map_err(tool_error)?;
            (None, request)
        } else {
            let name = invocation.tool_name.name.as_str();
            let (requirement, action) = match (name, &invocation.payload) {
                ("exec_command", ToolPayload::Function { arguments }) => {
                    let args: ToolEnvironmentArgs =
                        parse_arguments(arguments).map_err(tool_error)?;
                    let environment = resolve_tool_environment(
                        &step.environments,
                        args.environment_id.as_deref(),
                    )
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
                    let environment =
                        selected_executor(&invocation.environments, &step.environments)
                            .map_err(tool_error)?;
                    let prepared = prepare_patch_request(input, environment, step)
                        .await
                        .map_err(tool_error)?;
                    let action = prepared.approval_action(&invocation.call_id);
                    (Some(prepared.exec_approval_requirement), action)
                }
                ("request_permissions", ToolPayload::Function { arguments }) => {
                    let environment =
                        selected_executor(&invocation.environments, &step.environments)
                            .map_err(tool_error)?;
                    let prepared = PreparedPermissionRequest::parse(arguments, environment)
                        .map_err(tool_error)?;
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
                None | Some(ExecApprovalRequirement::Skip { .. }) => None,
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
            let request = from_approval_action(action, &context)
                .request
                .map_err(CodexErr::InvalidRequest)?;
            (approval, request)
        };
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
        Ok((invocation.call_id, input))
    };
    let review = async {
        let (call_id, assessment) = match mode {
            RecordedReviewMode::Transcript { root_evidence } => {
                before_deadline(deadline, async {
                    let (call_id, input) = resolve_recorded_call(root_evidence).await?;
                    let assessment = input
                        .assess_with_transcript_reviewer(deadline, cancellation)
                        .await
                        .map_err(assessment_error)?;
                    Ok((call_id, assessment))
                })
                .await?
            }
            RecordedReviewMode::Attached(route_pending_history) => {
                let (call_id, runtime, prepared) = before_deadline(deadline, async {
                    let (call_id, input) = resolve_recorded_call(/*root_evidence*/ None).await?;
                    let reviewer = session
                        .services
                        .thread_extension_data
                        .get::<Arc<dyn AttachedReviewer>>()
                        .ok_or_else(|| {
                            CodexErr::Fatal("Guardian reviewer is unavailable".into())
                        })?;
                    route_pending_history.await.map_err(|error| {
                        CodexErr::Fatal(format!("Guardian could not prepare the evidence: {error}"))
                    })?;
                    let (runtime, prepared) = prepare_attached_reviewer(
                        input,
                        Arc::clone(reviewer.as_ref()),
                        deadline,
                        cancellation,
                    )
                    .await?;
                    Ok((call_id, runtime, prepared))
                })
                .await?;
                let assessment =
                    assess_with_attached_reviewer(runtime, prepared, deadline, cancellation)
                        .await?;
                (call_id, assessment)
            }
        };
        Ok(GuardianReviewResult {
            call_id: call_id.into(),
            assessment,
        })
    };
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(CodexErr::TurnAborted),
        result = review => result,
    }
}

async fn before_deadline<T>(
    deadline: ReviewDeadline,
    work: impl Future<Output = Result<T>>,
) -> Result<T> {
    tokio::select! {
        biased;
        _ = deadline.wait() => Err(CodexErr::RequestTimeout),
        result = work => result,
    }
}

/// The attached reviewer reads live evidence, so recorded evals use a dedicated
/// thread and inject only between reviews.
async fn prepare_attached_reviewer(
    input: ReviewInput,
    reviewer: Arc<dyn AttachedReviewer>,
    deadline: ReviewDeadline,
    cancellation: &CancellationToken,
) -> Result<(ReviewRuntime, PreparedApproval)> {
    let (version, history_reset) = input.session.history_reset().await;
    if version != input.history.reset_version {
        return Err(CodexErr::TurnAborted);
    }
    let runtime = ReviewRuntime {
        attached_reviewer: Some(reviewer),
        session: input.session,
        history_reset,
        context: input.context,
        request: input.request.into(),
        reasons: input.reasons,
        options: GuardianReviewOptions {
            require_guardian: true,
            require_synchronous_review: true,
            plugin_attribution_override: None,
            approval_request_source: GuardianApprovalRequestSource::MainTurn,
            external_cancel: Some(cancellation.clone()),
        },
    };
    // A failure to prepare evidence is an RPC error, never a denial.
    let id = new_guardian_review_id();
    let (prepared, _) = runtime
        .prepare(&id, GuardianReviewReason::Policy, deadline, cancellation)
        .await
        .map_err(|decision| match decision {
            ReviewDecision::Abort => CodexErr::TurnAborted,
            ReviewDecision::Denied { rejection } => CodexErr::Fatal(rejection),
            other => CodexErr::Fatal(format!("Unexpected preparation decision: {other:?}")),
        })?;
    Ok((runtime, prepared))
}

/// Assess without publishing or caching approval.
async fn assess_with_attached_reviewer(
    runtime: ReviewRuntime,
    prepared: PreparedApproval,
    deadline: ReviewDeadline,
    cancellation: &CancellationToken,
) -> Result<GuardianAssessment> {
    // The verdict runner owns the same deadline. Let it drain a stalled
    // reconsideration and recover the first denial.
    let (outcome, _, _) = runtime.attempt(&prepared, deadline, cancellation).await;
    match outcome {
        GuardianReviewOutcome::Completed(assessment) => Ok(assessment),
        GuardianReviewOutcome::Error(GuardianReviewError::Session { message, .. }) => {
            Err(CodexErr::Fatal(message))
        }
        GuardianReviewOutcome::Error(error) => Err(assessment_error(error)),
        GuardianReviewOutcome::CachedApproval => Err(CodexErr::Fatal(
            "Expected a fresh Guardian assessment".into(),
        )),
    }
}

fn assessment_error(error: GuardianReviewError) -> CodexErr {
    match error {
        GuardianReviewError::Cancelled => CodexErr::TurnAborted,
        GuardianReviewError::Timeout => CodexErr::RequestTimeout,
        other => CodexErr::Fatal(format!("Guardian assessment failed: {other:?}")),
    }
}

pub(super) fn root_messages(evidence: &GuardianRootEvidence) -> Vec<GuardianRootMessage> {
    let mut messages = vec![GuardianRootMessage::RetainedContextScope];
    if evidence.missing_user_instructions {
        messages.push(GuardianRootMessage::IncompleteRootInstructions);
    }
    if evidence.missing_assistant_context {
        messages.push(GuardianRootMessage::IncompleteAssistantContext);
    }
    messages.extend(evidence.messages.iter().map(|entry| match entry {
        GuardianRootEvidenceMessage::User(text) => GuardianRootMessage::User(text.clone()),
        GuardianRootEvidenceMessage::Assistant(text) => {
            GuardianRootMessage::Assistant(text.clone())
        }
    }));
    messages
}

fn tool_error(error: FunctionCallError) -> CodexErr {
    match error {
        FunctionCallError::RespondToModel(message) => CodexErr::InvalidRequest(message),
        FunctionCallError::Fatal(message) => CodexErr::Fatal(message),
    }
}
