/*
Module: orchestrator

Central place for approvals + sandbox selection + retry semantics. Drives a
simple sequence for any ToolRuntime: approval → select sandbox → attempt →
retry with an escalated sandbox strategy on denial (no re‑approval thanks to
caching).
*/
use crate::guardian::guardian_rejection_message;
use crate::guardian::guardian_timeout_message;
use crate::guardian::new_guardian_review_id;
use crate::guardian::routes_approval_to_guardian;
use crate::network_policy_decision::network_approval_context_from_payload;
use crate::tools::approval_dispatch::request_approval;
use crate::tools::flat_tool_name;
use crate::tools::network_approval::ActiveNetworkApproval;
use crate::tools::network_approval::DeferredNetworkApproval;
use crate::tools::network_approval::NetworkApprovalMode;
use crate::tools::network_approval::begin_network_approval;
use crate::tools::network_approval::finish_deferred_network_approval;
use crate::tools::network_approval::finish_immediate_network_approval;
use crate::tools::sandboxing::ApprovalCtx;
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
use codex_protocol::error::CodexErr;
use codex_protocol::error::SandboxErr;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::NetworkPolicyRuleAction;
use codex_protocol::protocol::ReviewDecision;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxType;
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
        managed_network_active: bool,
    ) -> (Result<Out, ToolError>, Option<DeferredNetworkApproval>)
    where
        T: ToolRuntime<Rq, Out>,
    {
        let network_approval = begin_network_approval(
            &tool_ctx.session,
            &tool_ctx.turn.sub_id,
            managed_network_active,
            tool.network_approval_spec(req, tool_ctx),
        )
        .await;

        let attempt_tool_ctx = ToolCtx {
            session: tool_ctx.session.clone(),
            turn: tool_ctx.turn.clone(),
            call_id: tool_ctx.call_id.clone(),
            tool_name: tool_ctx.tool_name.clone(),
        };
        let attempt_with_network_approval = SandboxAttempt {
            sandbox: attempt.sandbox,
            permissions: attempt.permissions,
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
        };
        let run_result = tool
            .run(req, &attempt_with_network_approval, &attempt_tool_ctx)
            .await;

        let Some(network_approval) = network_approval else {
            return (run_result, None);
        };

        match network_approval.mode() {
            NetworkApprovalMode::Immediate => {
                let finalize_result =
                    finish_immediate_network_approval(&tool_ctx.session, network_approval).await;
                if let Err(err) = finalize_result {
                    return (Err(err), None);
                }
                (run_result, None)
            }
            NetworkApprovalMode::Deferred => {
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
        }
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
        let strict_auto_review = tool_ctx.session.strict_auto_review_enabled_for_turn().await;
        let use_guardian = routes_approval_to_guardian(turn_ctx) || strict_auto_review;

        // 1) Approval
        let mut already_approved = false;

        let file_system_sandbox_policy = turn_ctx.file_system_sandbox_policy();
        let network_sandbox_policy = turn_ctx.network_sandbox_policy();
        let requirement = tool.exec_approval_requirement(req).unwrap_or_else(|| {
            default_exec_approval_requirement(approval_policy, &file_system_sandbox_policy)
        });
        match &requirement {
            ExecApprovalRequirement::Skip { .. } => {
                if strict_auto_review {
                    let guardian_review_id = Some(new_guardian_review_id());
                    let approval_ctx = ApprovalCtx {
                        session: &tool_ctx.session,
                        turn: &tool_ctx.turn,
                        call_id: &tool_ctx.call_id,
                        guardian_review_id: guardian_review_id.clone(),
                        retry_reason: None,
                        network_approval_context: None,
                    };
                    let decision = request_approval(
                        tool,
                        req,
                        tool_ctx.call_id.as_str(),
                        approval_ctx,
                        tool_ctx,
                        /*evaluate_permission_request_hooks*/ false,
                        &otel,
                    )
                    .await?;
                    Self::reject_if_not_approved(tool_ctx, guardian_review_id.as_deref(), decision)
                        .await?;
                    already_approved = true;
                } else {
                    otel.tool_decision(
                        &otel_tn,
                        otel_ci,
                        &ReviewDecision::Approved,
                        ToolDecisionSource::Config,
                    );
                }
            }
            ExecApprovalRequirement::Forbidden { reason } => {
                return Err(ToolError::Rejected(reason.clone()));
            }
            ExecApprovalRequirement::NeedsApproval { reason, .. } => {
                let guardian_review_id = use_guardian.then(new_guardian_review_id);
                let approval_ctx = ApprovalCtx {
                    session: &tool_ctx.session,
                    turn: &tool_ctx.turn,
                    call_id: &tool_ctx.call_id,
                    guardian_review_id: guardian_review_id.clone(),
                    retry_reason: reason.clone(),
                    network_approval_context: None,
                };
                let decision = request_approval(
                    tool,
                    req,
                    tool_ctx.call_id.as_str(),
                    approval_ctx,
                    tool_ctx,
                    /*evaluate_permission_request_hooks*/ !strict_auto_review,
                    &otel,
                )
                .await?;

                Self::reject_if_not_approved(tool_ctx, guardian_review_id.as_deref(), decision)
                    .await?;
                already_approved = true;
            }
        }

        // 2) First attempt under the selected sandbox.
        let sandbox_override = sandbox_override_for_first_attempt(
            tool.sandbox_permissions(req),
            &requirement,
            &file_system_sandbox_policy,
        );
        let managed_network_active = turn_ctx.network.is_some();
        let initial_sandbox = match sandbox_override {
            SandboxOverride::BypassSandboxFirstAttempt => SandboxType::None,
            SandboxOverride::NoOverride => self.sandbox.select_initial(
                &file_system_sandbox_policy,
                network_sandbox_policy,
                tool.sandbox_preference(),
                turn_ctx.windows_sandbox_level,
                managed_network_active,
            ),
        };

        // Platform-specific flag gating is handled by SandboxManager::select_initial.
        let use_legacy_landlock = turn_ctx.features.use_legacy_landlock();
        #[allow(deprecated)]
        let sandbox_cwd = tool.sandbox_cwd(req).unwrap_or(&turn_ctx.cwd);
        let workspace_roots = turn_ctx.config.effective_workspace_roots();
        let initial_attempt = SandboxAttempt {
            sandbox: initial_sandbox,
            permissions: &turn_ctx.permission_profile,
            enforce_managed_network: managed_network_active,
            manager: &self.sandbox,
            sandbox_cwd,
            workspace_roots: workspace_roots.as_slice(),
            codex_linux_sandbox_exe: turn_ctx.codex_linux_sandbox_exe.as_ref(),
            use_legacy_landlock,
            windows_sandbox_level: turn_ctx.windows_sandbox_level,
            windows_sandbox_private_desktop: turn_ctx
                .config
                .permissions
                .windows_sandbox_private_desktop,
            network_denial_cancellation_token: None,
        };

        let initial_attempt_start = Instant::now();
        let (first_result, first_deferred_network_approval) = Self::run_attempt(
            tool,
            req,
            tool_ctx,
            &initial_attempt,
            managed_network_active,
        )
        .await;
        let initial_duration = initial_attempt_start.elapsed();
        match first_result {
            Ok(out) => {
                // We have a successful initial result
                Ok(OrchestratorRunResult {
                    output: out,
                    deferred_network_approval: first_deferred_network_approval,
                })
            }
            Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                output,
                network_policy_decision,
            }))) => {
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
                    return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                        output,
                        network_policy_decision,
                    })));
                }
                if !tool.escalate_on_failure() {
                    otel.sandbox_outcome(
                        &otel_tn,
                        otel_ci,
                        "denied",
                        initial_duration,
                        /*escalated_duration*/ None,
                    );
                    return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                        output,
                        network_policy_decision,
                    })));
                }
                let unsandboxed_allowed =
                    unsandboxed_execution_allowed(&file_system_sandbox_policy);
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
                        return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                            output,
                            network_policy_decision,
                        })));
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
                    return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                        output,
                        network_policy_decision,
                    })));
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
                    let guardian_review_id = use_guardian.then(new_guardian_review_id);
                    let approval_ctx = ApprovalCtx {
                        session: &tool_ctx.session,
                        turn: &tool_ctx.turn,
                        call_id: &tool_ctx.call_id,
                        guardian_review_id: guardian_review_id.clone(),
                        retry_reason: Some(retry_reason),
                        network_approval_context: network_approval_context.clone(),
                    };

                    let permission_request_run_id = format!("{}:retry", tool_ctx.call_id);
                    let decision = request_approval(
                        tool,
                        req,
                        &permission_request_run_id,
                        approval_ctx,
                        tool_ctx,
                        /*evaluate_permission_request_hooks*/ !strict_auto_review,
                        &otel,
                    )
                    .await?;

                    Self::reject_if_not_approved(tool_ctx, guardian_review_id.as_deref(), decision)
                        .await?;
                }

                let retry_sandbox = if unsandboxed_allowed {
                    SandboxType::None
                } else {
                    self.sandbox.select_initial(
                        &file_system_sandbox_policy,
                        network_sandbox_policy,
                        tool.sandbox_preference(),
                        turn_ctx.windows_sandbox_level,
                        managed_network_active,
                    )
                };
                let retry_codex_linux_sandbox_exe = if unsandboxed_allowed {
                    None
                } else {
                    turn_ctx.codex_linux_sandbox_exe.as_ref()
                };
                let retry_attempt = SandboxAttempt {
                    sandbox: retry_sandbox,
                    permissions: &turn_ctx.permission_profile,
                    enforce_managed_network: managed_network_active,
                    manager: &self.sandbox,
                    sandbox_cwd,
                    workspace_roots: workspace_roots.as_slice(),
                    codex_linux_sandbox_exe: retry_codex_linux_sandbox_exe,
                    use_legacy_landlock,
                    windows_sandbox_level: turn_ctx.windows_sandbox_level,
                    windows_sandbox_private_desktop: turn_ctx
                        .config
                        .permissions
                        .windows_sandbox_private_desktop,
                    network_denial_cancellation_token: None,
                };

                // Second attempt.
                let escalated_attempt_start = Instant::now();
                let (retry_result, retry_deferred_network_approval) =
                    Self::run_attempt(tool, req, tool_ctx, &retry_attempt, managed_network_active)
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

    async fn reject_if_not_approved(
        tool_ctx: &ToolCtx,
        guardian_review_id: Option<&str>,
        decision: ReviewDecision,
    ) -> Result<(), ToolError> {
        match decision {
            ReviewDecision::Denied | ReviewDecision::Abort => {
                let reason = if let Some(review_id) = guardian_review_id {
                    guardian_rejection_message(tool_ctx.turn.as_ref(), review_id).await
                } else {
                    "rejected by user".to_string()
                };
                Err(ToolError::Rejected(reason))
            }
            ReviewDecision::TimedOut => Err(ToolError::Rejected(guardian_timeout_message())),
            ReviewDecision::Approved
            | ReviewDecision::ApprovedExecpolicyAmendment { .. }
            | ReviewDecision::ApprovedForSession => Ok(()),
            ReviewDecision::NetworkPolicyAmendment {
                network_policy_amendment,
            } => match network_policy_amendment.action {
                NetworkPolicyRuleAction::Allow => Ok(()),
                NetworkPolicyRuleAction::Deny => {
                    Err(ToolError::Rejected("rejected by user".to_string()))
                }
            },
        }
    }
}

fn sandbox_outcome_from_tool_error(err: &ToolError) -> Option<&'static str> {
    match err {
        ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied { .. })) => Some("denied"),
        ToolError::Codex(CodexErr::Sandbox(SandboxErr::Timeout { .. })) => Some("timed_out"),
        ToolError::Codex(CodexErr::Sandbox(SandboxErr::Signal(_))) => Some("signal"),
        ToolError::Rejected(_) | ToolError::Codex(_) => None,
    }
}

fn build_denial_reason_from_output(_output: &ExecToolCallOutput) -> String {
    // Keep approval reason terse and stable for UX/tests, but accept the
    // output so we can evolve heuristics later without touching call sites.
    "command failed; retry without sandbox?".to_string()
}
