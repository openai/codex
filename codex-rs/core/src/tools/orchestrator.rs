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
use crate::hook_runtime::run_permission_request_hooks;
use crate::network_policy_decision::network_approval_context_from_payload;
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
use codex_hooks::PermissionRequestDecision;
use codex_otel::ToolDecisionSource;
use codex_protocol::error::CodexErr;
use codex_protocol::error::SandboxErr;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::NetworkPolicyRuleAction;
use codex_protocol::protocol::ReviewDecision;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxType;

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
        let run_result = tool.run(req, attempt, &attempt_tool_ctx).await;

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
                    finish_deferred_network_approval(&tool_ctx.session, deferred).await;
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
        let otel_tn = &tool_ctx.tool_name;
        let otel_ci = &tool_ctx.call_id;
        let strict_auto_review = tool_ctx.session.strict_auto_review_enabled_for_turn().await;
        let use_guardian = routes_approval_to_guardian(turn_ctx) || strict_auto_review;

        // 1) Approval
        let mut already_approved = false;

        let requirement = tool.exec_approval_requirement(req).unwrap_or_else(|| {
            default_exec_approval_requirement(approval_policy, &turn_ctx.file_system_sandbox_policy)
        });
        match requirement {
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
                    let decision = Self::request_approval(
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
                        otel_tn,
                        otel_ci,
                        &ReviewDecision::Approved,
                        ToolDecisionSource::Config,
                    );
                }
            }
            ExecApprovalRequirement::Forbidden { reason } => {
                return Err(ToolError::Rejected(reason));
            }
            ExecApprovalRequirement::NeedsApproval { reason, .. } => {
                let guardian_review_id = use_guardian.then(new_guardian_review_id);
                let approval_ctx = ApprovalCtx {
                    session: &tool_ctx.session,
                    turn: &tool_ctx.turn,
                    call_id: &tool_ctx.call_id,
                    guardian_review_id: guardian_review_id.clone(),
                    retry_reason: reason,
                    network_approval_context: None,
                };
                let decision = Self::request_approval(
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
        let managed_network_active = turn_ctx.network.is_some();
        let initial_sandbox = match tool.sandbox_mode_for_first_attempt(req) {
            SandboxOverride::BypassSandboxFirstAttempt => SandboxType::None,
            SandboxOverride::NoOverride => self.sandbox.select_initial(
                &turn_ctx.file_system_sandbox_policy,
                turn_ctx.network_sandbox_policy,
                tool.sandbox_preference(),
                turn_ctx.windows_sandbox_level,
                managed_network_active,
            ),
        };

        // Platform-specific flag gating is handled by SandboxManager::select_initial.
        let use_legacy_landlock = turn_ctx.features.use_legacy_landlock();
        let initial_attempt = SandboxAttempt {
            sandbox: initial_sandbox,
            policy: &turn_ctx.sandbox_policy,
            file_system_policy: &turn_ctx.file_system_sandbox_policy,
            network_policy: turn_ctx.network_sandbox_policy,
            enforce_managed_network: managed_network_active,
            manager: &self.sandbox,
            sandbox_cwd: &turn_ctx.cwd,
            codex_linux_sandbox_exe: turn_ctx.codex_linux_sandbox_exe.as_ref(),
            use_legacy_landlock,
            windows_sandbox_level: turn_ctx.windows_sandbox_level,
            windows_sandbox_private_desktop: turn_ctx
                .config
                .permissions
                .windows_sandbox_private_desktop,
        };

        let (first_result, first_deferred_network_approval) = Self::run_attempt(
            tool,
            req,
            tool_ctx,
            &initial_attempt,
            managed_network_active,
        )
        .await;
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
                    return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                        output,
                        network_policy_decision,
                    })));
                }
                if !tool.escalate_on_failure() {
                    return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                        output,
                        network_policy_decision,
                    })));
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
                                    &turn_ctx.file_system_sandbox_policy
                                ),
                                ExecApprovalRequirement::NeedsApproval { .. }
                            );
                    if !allow_on_request_network_prompt {
                        return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                            output,
                            network_policy_decision,
                        })));
                    }
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

                // Ask for approval before retrying with the escalated sandbox.
                let bypass_retry_approval = tool
                    .should_bypass_approval(approval_policy, already_approved)
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
                    let decision = Self::request_approval(
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

                let escalated_attempt = SandboxAttempt {
                    sandbox: SandboxType::None,
                    policy: &turn_ctx.sandbox_policy,
                    file_system_policy: &turn_ctx.file_system_sandbox_policy,
                    network_policy: turn_ctx.network_sandbox_policy,
                    enforce_managed_network: managed_network_active,
                    manager: &self.sandbox,
                    sandbox_cwd: &turn_ctx.cwd,
                    codex_linux_sandbox_exe: None,
                    use_legacy_landlock,
                    windows_sandbox_level: turn_ctx.windows_sandbox_level,
                    windows_sandbox_private_desktop: turn_ctx
                        .config
                        .permissions
                        .windows_sandbox_private_desktop,
                };

                // Second attempt.
                let (retry_result, retry_deferred_network_approval) = Self::run_attempt(
                    tool,
                    req,
                    tool_ctx,
                    &escalated_attempt,
                    managed_network_active,
                )
                .await;
                retry_result.map(|output| OrchestratorRunResult {
                    output,
                    deferred_network_approval: retry_deferred_network_approval,
                })
            }
            Err(err) => Err(err),
        }
    }

    // PermissionRequest hooks take top precedence for answering approval
    // prompts. If no matching hook returns a decision, fall back to the
    // normal guardian or user approval path.
    async fn request_approval<Rq, Out, T>(
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
                        &tool_ctx.tool_name,
                        &tool_ctx.call_id,
                        &decision,
                        ToolDecisionSource::Config,
                    );
                    return Ok(decision);
                }
                Some(PermissionRequestDecision::Deny { message }) => {
                    let decision = ReviewDecision::Denied;
                    otel.tool_decision(
                        &tool_ctx.tool_name,
                        &tool_ctx.call_id,
                        &decision,
                        ToolDecisionSource::Config,
                    );
                    return Err(ToolError::Rejected(message));
                }
                None => {}
            }
        }

        let otel_source = if approval_ctx.guardian_review_id.is_some() {
            ToolDecisionSource::AutomatedReviewer
        } else {
            ToolDecisionSource::User
        };
        let decision = tool.start_approval_async(req, approval_ctx).await;
        otel.tool_decision(
            &tool_ctx.tool_name,
            &tool_ctx.call_id,
            &decision,
            otel_source,
        );
        Ok(decision)
    }

    async fn reject_if_not_approved(
        tool_ctx: &ToolCtx,
        guardian_review_id: Option<&str>,
        decision: ReviewDecision,
    ) -> Result<(), ToolError> {
        match decision {
            ReviewDecision::Denied | ReviewDecision::Abort => {
                let reason = if let Some(review_id) = guardian_review_id {
                    guardian_rejection_message(tool_ctx.session.as_ref(), review_id).await
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

fn build_denial_reason_from_output(_output: &ExecToolCallOutput) -> String {
    // Keep approval reason terse and stable for UX/tests, but accept the
    // output so we can evolve heuristics later without touching call sites.
    "command failed; retry without sandbox?".to_string()
}
