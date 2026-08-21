/*
Module: orchestrator

Central place for approvals + sandbox selection + retry semantics. Drives a
simple sequence for any ToolRuntime: approval → select sandbox → attempt →
retry with an escalated sandbox strategy on denial (no re‑approval thanks to
caching).
*/
use crate::config::NetworkProxySpec;
use crate::guardian::GuardianReviewContext;
use crate::network_policy_decision::network_approval_context_from_payload;
use crate::tools::approvals::ApprovalContext;
use crate::tools::flat_tool_name;
use crate::tools::network_approval::ActiveNetworkApproval;
use crate::tools::network_approval::DeferredNetworkApproval;
use crate::tools::network_approval::NetworkApprovalSpec;
use crate::tools::network_approval::begin_network_approval;
use crate::tools::network_approval::finish_deferred_network_approval;
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::SandboxOverride;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::default_exec_approval_requirement;
use crate::tools::sandboxing::sandbox_override_for_first_attempt;
use crate::tools::sandboxing::unsandboxed_execution_allowed;
use codex_otel::ToolDecisionSource;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::SandboxErr;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxType;
use codex_sandboxing::policy_transforms::effective_network_sandbox_policy;
use std::sync::Arc;
use std::time::Instant;

pub(crate) struct ToolOrchestrator {
    sandbox: SandboxManager,
}

pub(crate) struct OrchestratorRunResult<Out> {
    pub output: Out,
    pub deferred_network_approval: Option<DeferredNetworkApproval>,
}

impl ToolOrchestrator {
    pub fn new() -> Self {
        Self {
            sandbox: SandboxManager::new(),
        }
    }

    async fn run_attempt<Rq, Out, T>(
        tool: &mut T,
        req: &Rq,
        tool_ctx: &ToolCtx,
        attempt: &SandboxAttempt<'_>,
        network_approval_spec: Option<NetworkApprovalSpec>,
    ) -> (Result<Out, ToolError>, Option<DeferredNetworkApproval>)
    where
        T: ToolRuntime<Rq, Out>,
    {
        let network_approval = match begin_network_approval(
            &tool_ctx.session,
            &tool_ctx.step_context.turn,
            attempt.enforce_managed_network,
            network_approval_spec,
        )
        .await
        {
            Ok(network_approval) => network_approval,
            Err(err) => return (Err(err), None),
        };

        let attempt_tool_ctx = ToolCtx {
            session: tool_ctx.session.clone(),
            step_context: Arc::clone(&tool_ctx.step_context),
            cancellation_token: tool_ctx.cancellation_token.clone(),
            call_id: tool_ctx.call_id.clone(),
            tool_name: tool_ctx.tool_name.clone(),
        };
        let attempt_with_network_approval = SandboxAttempt {
            sandbox: attempt.sandbox,
            sandbox_requested: attempt.sandbox_requested,
            permissions: attempt.permissions,
            exec_server_permissions: attempt.exec_server_permissions,
            enforce_managed_network: attempt.enforce_managed_network,
            manager: attempt.manager,
            sandbox_cwd: attempt.sandbox_cwd,
            workspace_roots: attempt.workspace_roots,
            codex_linux_sandbox_exe: attempt.codex_linux_sandbox_exe,
            use_legacy_landlock: attempt.use_legacy_landlock,
            windows_sandbox_level: attempt.windows_sandbox_level,
            windows_sandbox_private_desktop: attempt.windows_sandbox_private_desktop,
            network_denial_cancellation_token: network_approval
                .as_ref()
                .map(ActiveNetworkApproval::cancellation_token),
            network_proxy: network_approval
                .as_ref()
                .map(ActiveNetworkApproval::execution_proxy),
        };
        let run_result = tool
            .run(req, &attempt_with_network_approval, &attempt_tool_ctx)
            .await;

        let Some(network_approval) = network_approval else {
            return (run_result, None);
        };

        let deferred = network_approval.into_deferred();
        if run_result.is_err() {
            let finalize_result =
                finish_deferred_network_approval(&tool_ctx.session, deferred).await;
            if let Err(err) = finalize_result {
                return (Err(err), None);
            }
            return (run_result, None);
        }
        (run_result, deferred)
    }

    pub async fn run<Rq, Out, T>(
        &mut self,
        tool: &mut T,
        req: &Rq,
        tool_ctx: &ToolCtx,
        turn_ctx: &crate::session::turn_context::TurnContext,
        approval_policy: AskForApproval,
    ) -> Result<OrchestratorRunResult<Out>, ToolError>
    where
        T: ToolRuntime<Rq, Out>,
    {
        let otel = turn_ctx.session_telemetry.clone();
        let otel_tn = flat_tool_name(&tool_ctx.tool_name).into_owned();
        let otel_ci = &tool_ctx.call_id;
        let strict_auto_review = tool_ctx
            .session
            .active_turn_context_and_strict_auto_review()
            .await
            .is_some_and(|(_, strict_auto_review)| strict_auto_review);
        // 1) Approval
        let mut already_approved = false;

        let environment = tool.turn_environment(req);
        let owner_network_policy = environment.config().network_policy.is_some();
        if owner_network_policy
            && tool
                .sandbox_permissions(req)
                .requires_escalated_permissions()
        {
            return Err(ToolError::Rejected(
                "attachment-owned network policy cannot be bypassed by sandbox escalation"
                    .to_string(),
            ));
        }
        let workspace_roots = environment.workspace_roots();
        let executor_managed_process_sandbox = tool.uses_executor_managed_process_sandbox(req);
        let permission_profile = environment.permission_profile();
        let permissions = if executor_managed_process_sandbox {
            // Executor-native roots remain symbolic until the executor applies its own sandbox.
            permission_profile.clone()
        } else {
            environment.permission_profile_with_workspace_roots()
        };
        let file_system_sandbox_policy = permissions.file_system_sandbox_policy();
        let requirement = tool.exec_approval_requirement(req).unwrap_or_else(|| {
            default_exec_approval_requirement(approval_policy, &file_system_sandbox_policy)
        });
        match &requirement {
            ExecApprovalRequirement::Skip { .. } => {
                if strict_auto_review {
                    let action = tool
                        .approval_action(req, &tool_ctx.call_id)
                        .map_err(|err| {
                            ToolError::Rejected(format!("could not prepare approval action: {err}"))
                        })?;
                    let approval_ctx = ApprovalContext {
                        review_context: GuardianReviewContext::from(&tool_ctx.step_context),
                        cancellation_token: Some(tool_ctx.cancellation_token.clone()),
                        call_id: tool_ctx.call_id.clone(),
                        tool_name: tool_ctx.tool_name.clone(),
                        strict_auto_review,
                        approval_reason: None,
                        retry_reason: None,
                        network_approval_context: None,
                    };
                    tool_ctx
                        .session
                        .request_approval(action, approval_ctx)
                        .await?;
                    already_approved = true;
                } else {
                    otel.tool_decision(
                        &tool_ctx.tool_name,
                        otel_ci,
                        &ReviewDecision::Approved,
                        Some(ToolDecisionSource::Config),
                    );
                }
            }
            ExecApprovalRequirement::Forbidden { reason } => {
                return Err(ToolError::Rejected(reason.clone()));
            }
            ExecApprovalRequirement::NeedsApproval { reason, .. } => {
                let action = tool
                    .approval_action(req, &tool_ctx.call_id)
                    .map_err(|err| {
                        ToolError::Rejected(format!("could not prepare approval action: {err}"))
                    })?;
                let approval_ctx = ApprovalContext {
                    review_context: GuardianReviewContext::from(&tool_ctx.step_context),
                    cancellation_token: Some(tool_ctx.cancellation_token.clone()),
                    call_id: tool_ctx.call_id.clone(),
                    tool_name: tool_ctx.tool_name.clone(),
                    strict_auto_review,
                    approval_reason: reason.clone(),
                    retry_reason: None,
                    network_approval_context: None,
                };
                tool_ctx
                    .session
                    .request_approval(action, approval_ctx)
                    .await?;
                already_approved = true;
            }
        }

        // 2) First attempt under the selected sandbox.
        let unsandboxed_allowed =
            !owner_network_policy && unsandboxed_execution_allowed(&file_system_sandbox_policy);
        let sandbox_override = if unsandboxed_allowed {
            sandbox_override_for_first_attempt(
                tool.sandbox_permissions(req),
                &requirement,
                &file_system_sandbox_policy,
            )
        } else {
            SandboxOverride::NoOverride
        };
        let network_approval_spec = tool.network_approval_spec(req, tool_ctx);
        // Offline owner attachments stay offline unless approved command permissions grant
        // networking. Existing enabled controller proxies remain independently authoritative.
        let managed_network_active = if owner_network_policy {
            turn_ctx
                .config
                .permissions
                .network
                .as_ref()
                .is_some_and(NetworkProxySpec::enabled)
                || network_approval_spec.as_ref().is_some_and(|spec| {
                    effective_network_sandbox_policy(
                        permission_profile.network_sandbox_policy(),
                        spec.trigger.additional_permissions.as_ref(),
                    )
                    .is_enabled()
                })
        } else {
            turn_ctx.network.is_some()
        };
        let sandbox_preference = tool.sandbox_preference();
        let sandbox_requested = match sandbox_override {
            SandboxOverride::BypassSandboxFirstAttempt => false,
            SandboxOverride::NoOverride => self.sandbox.should_sandbox(
                &permissions,
                sandbox_preference,
                managed_network_active,
            ),
        };
        let initial_sandbox = if sandbox_requested && !executor_managed_process_sandbox {
            self.sandbox.select_initial(
                &permissions,
                sandbox_preference,
                turn_ctx.windows_sandbox_level,
                managed_network_active,
            )
        } else {
            SandboxType::None
        };

        // Platform-specific flag gating is handled by SandboxManager::select_initial.
        let use_legacy_landlock = turn_ctx.config.features.use_legacy_landlock();
        let sandbox_policy_cwd = tool
            .sandbox_cwd(req)
            .cloned()
            .unwrap_or_else(|| environment.cwd().clone());
        let initial_attempt = SandboxAttempt {
            sandbox: initial_sandbox,
            sandbox_requested,
            permissions: &permissions,
            exec_server_permissions: permission_profile,
            enforce_managed_network: managed_network_active,
            manager: &self.sandbox,
            sandbox_cwd: &sandbox_policy_cwd,
            workspace_roots,
            codex_linux_sandbox_exe: turn_ctx.config.codex_linux_sandbox_exe.as_ref(),
            use_legacy_landlock,
            windows_sandbox_level: turn_ctx.windows_sandbox_level,
            windows_sandbox_private_desktop: turn_ctx
                .config
                .permissions
                .windows_sandbox_private_desktop,
            network_denial_cancellation_token: None,
            network_proxy: None,
        };

        let initial_attempt_start = Instant::now();
        let (first_result, first_deferred_network_approval) =
            Self::run_attempt(tool, req, tool_ctx, &initial_attempt, network_approval_spec).await;
        let initial_duration = initial_attempt_start.elapsed();
        match first_result {
            Ok(out) => {
                // We have a successful initial result
                Ok(OrchestratorRunResult {
                    output: out,
                    deferred_network_approval: first_deferred_network_approval,
                })
            }
            Err(ToolError::Codex(err)) => {
                let CodexErrorDetails::Sandbox(SandboxErr::Denied {
                    output,
                    network_policy_decision,
                }) = err.details()
                else {
                    let err = ToolError::Codex(err);
                    if let Some(outcome) = sandbox_outcome_from_tool_error(&err) {
                        otel.sandbox_outcome(
                            &otel_tn,
                            otel_ci,
                            outcome,
                            initial_duration,
                            /*escalated_duration*/ None,
                        );
                    }
                    return Err(err);
                };
                let network_approval_context = if managed_network_active {
                    network_policy_decision
                        .as_ref()
                        .and_then(network_approval_context_from_payload)
                } else {
                    None
                };
                if network_policy_decision.is_some() && network_approval_context.is_none() {
                    otel.sandbox_outcome(
                        &otel_tn,
                        otel_ci,
                        "denied",
                        initial_duration,
                        /*escalated_duration*/ None,
                    );
                    return Err(ToolError::Codex(err));
                }
                if !tool.escalate_on_failure() {
                    otel.sandbox_outcome(
                        &otel_tn,
                        otel_ci,
                        "denied",
                        initial_duration,
                        /*escalated_duration*/ None,
                    );
                    return Err(ToolError::Codex(err));
                }
                // Under `Never` or `OnRequest`, do not retry without sandbox;
                // surface a concise sandbox denial that preserves the
                // original output.
                if !tool.wants_no_sandbox_approval(approval_policy) {
                    let allow_on_request_network_prompt =
                        matches!(approval_policy, AskForApproval::OnRequest)
                            && network_approval_context.is_some()
                            && matches!(
                                default_exec_approval_requirement(
                                    approval_policy,
                                    &file_system_sandbox_policy
                                ),
                                ExecApprovalRequirement::NeedsApproval { .. }
                            );
                    if !allow_on_request_network_prompt {
                        otel.sandbox_outcome(
                            &otel_tn,
                            otel_ci,
                            "denied",
                            initial_duration,
                            /*escalated_duration*/ None,
                        );
                        return Err(ToolError::Codex(err));
                    }
                }
                if !unsandboxed_allowed && network_approval_context.is_none() {
                    otel.sandbox_outcome(
                        &otel_tn,
                        otel_ci,
                        "denied",
                        initial_duration,
                        /*escalated_duration*/ None,
                    );
                    return Err(ToolError::Codex(err));
                }
                let retry_reason =
                    if let Some(network_approval_context) = network_approval_context.as_ref() {
                        format!(
                            "Network access to \"{}\" is blocked by policy.",
                            network_approval_context.host
                        )
                    } else {
                        build_denial_reason_from_output(output.as_ref())
                    };

                // Strict auto-review approval covers the sandboxed attempt only;
                // retrying without the sandbox requires a fresh guardian review.
                let bypass_retry_approval = !strict_auto_review
                    && tool.should_bypass_approval(approval_policy, already_approved)
                    && network_approval_context.is_none();
                if !bypass_retry_approval {
                    let approval_reason = match &requirement {
                        ExecApprovalRequirement::NeedsApproval { reason, .. } => reason.clone(),
                        ExecApprovalRequirement::Skip { .. }
                        | ExecApprovalRequirement::Forbidden { .. } => None,
                    };
                    let action = tool
                        .approval_action(req, &tool_ctx.call_id)
                        .map_err(|err| {
                            ToolError::Rejected(format!("could not prepare approval action: {err}"))
                        })?;
                    let approval_ctx = ApprovalContext {
                        review_context: GuardianReviewContext::from(&tool_ctx.step_context),
                        cancellation_token: Some(tool_ctx.cancellation_token.clone()),
                        call_id: tool_ctx.call_id.clone(),
                        tool_name: tool_ctx.tool_name.clone(),
                        strict_auto_review,
                        approval_reason,
                        retry_reason: Some(retry_reason),
                        network_approval_context: network_approval_context.clone(),
                    };

                    tool_ctx
                        .session
                        .request_approval(action, approval_ctx)
                        .await?;
                }

                let retry_sandbox_requested = !unsandboxed_allowed
                    && self.sandbox.should_sandbox(
                        &permissions,
                        sandbox_preference,
                        managed_network_active,
                    );
                let retry_sandbox = if retry_sandbox_requested && !executor_managed_process_sandbox
                {
                    self.sandbox.select_initial(
                        &permissions,
                        sandbox_preference,
                        turn_ctx.windows_sandbox_level,
                        managed_network_active,
                    )
                } else {
                    SandboxType::None
                };
                let retry_codex_linux_sandbox_exe = if unsandboxed_allowed {
                    None
                } else {
                    turn_ctx.config.codex_linux_sandbox_exe.as_ref()
                };
                let retry_attempt = SandboxAttempt {
                    sandbox: retry_sandbox,
                    sandbox_requested: retry_sandbox_requested,
                    permissions: &permissions,
                    exec_server_permissions: permission_profile,
                    enforce_managed_network: managed_network_active,
                    manager: &self.sandbox,
                    sandbox_cwd: &sandbox_policy_cwd,
                    workspace_roots,
                    codex_linux_sandbox_exe: retry_codex_linux_sandbox_exe,
                    use_legacy_landlock,
                    windows_sandbox_level: turn_ctx.windows_sandbox_level,
                    windows_sandbox_private_desktop: turn_ctx
                        .config
                        .permissions
                        .windows_sandbox_private_desktop,
                    network_denial_cancellation_token: None,
                    network_proxy: None,
                };

                // Second attempt.
                let network_approval_spec = tool.network_approval_spec(req, tool_ctx);
                let escalated_attempt_start = Instant::now();
                let (retry_result, retry_deferred_network_approval) =
                    Self::run_attempt(tool, req, tool_ctx, &retry_attempt, network_approval_spec)
                        .await;
                let escalated_duration = escalated_attempt_start.elapsed();
                match retry_result {
                    Ok(output) => {
                        otel.sandbox_outcome(
                            &otel_tn,
                            otel_ci,
                            "escalated",
                            initial_duration,
                            Some(escalated_duration),
                        );
                        Ok(OrchestratorRunResult {
                            output,
                            deferred_network_approval: retry_deferred_network_approval,
                        })
                    }
                    Err(err) => {
                        if let Some(outcome) = sandbox_outcome_from_tool_error(&err) {
                            otel.sandbox_outcome(
                                &otel_tn,
                                otel_ci,
                                outcome,
                                initial_duration,
                                Some(escalated_duration),
                            );
                        }
                        Err(err)
                    }
                }
            }
            Err(err) => {
                if let Some(outcome) = sandbox_outcome_from_tool_error(&err) {
                    otel.sandbox_outcome(
                        &otel_tn,
                        otel_ci,
                        outcome,
                        initial_duration,
                        /*escalated_duration*/ None,
                    );
                }
                Err(err)
            }
        }
    }
}

fn sandbox_outcome_from_tool_error(err: &ToolError) -> Option<&'static str> {
    match err {
        ToolError::Codex(err) => match err.details() {
            CodexErrorDetails::Sandbox(SandboxErr::Denied { .. }) => Some("denied"),
            CodexErrorDetails::Sandbox(SandboxErr::Timeout { .. }) => Some("timed_out"),
            CodexErrorDetails::Sandbox(SandboxErr::Signal(_)) => Some("signal"),
            _ => None,
        },
        ToolError::Rejected(_) => None,
    }
}

fn build_denial_reason_from_output(_output: &ExecToolCallOutput) -> String {
    // Keep approval reason terse and stable for UX/tests, but accept the
    // output so we can evolve heuristics later without touching call sites.
    "command failed; retry without sandbox?".to_string()
}
