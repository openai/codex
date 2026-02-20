/*
Module: orchestrator

Central place for approvals + sandbox selection + retry semantics. Drives a
simple sequence for any ToolRuntime: approval → select sandbox → attempt →
retry with an escalated sandbox strategy on denial (no re‑approval thanks to
caching).
*/
use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::exec::ExecToolCallOutput;
use crate::features::Feature;
use crate::network_policy_decision::network_approval_context_from_payload;
use crate::sandboxing::SandboxManager;
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
use codex_otel::ToolDecisionSource;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;

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
        tool_ctx: &ToolCtx<'_>,
        attempt: &SandboxAttempt<'_>,
        has_managed_network_requirements: bool,
    ) -> (Result<Out, ToolError>, Option<DeferredNetworkApproval>)
    where
        T: ToolRuntime<Rq, Out>,
    {
        let network_approval = begin_network_approval(
            tool_ctx.session,
            &tool_ctx.turn.sub_id,
            &tool_ctx.call_id,
            has_managed_network_requirements,
            tool.network_approval_spec(req, tool_ctx),
        )
        .await;

        let attempt_tool_ctx = ToolCtx {
            session: tool_ctx.session,
            turn: tool_ctx.turn,
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
                    finish_immediate_network_approval(tool_ctx.session, network_approval).await;
                if let Err(err) = finalize_result {
                    return (Err(err), None);
                }
                (run_result, None)
            }
            NetworkApprovalMode::Deferred => {
                let deferred = network_approval.into_deferred();
                if run_result.is_err() {
                    finish_deferred_network_approval(tool_ctx.session, deferred).await;
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
        tool_ctx: &ToolCtx<'_>,
        turn_ctx: &crate::codex::TurnContext,
        approval_policy: AskForApproval,
    ) -> Result<OrchestratorRunResult<Out>, ToolError>
    where
        T: ToolRuntime<Rq, Out>,
    {
        let otel = turn_ctx.otel_manager.clone();
        let otel_tn = &tool_ctx.tool_name;
        let otel_ci = &tool_ctx.call_id;
        let otel_user = ToolDecisionSource::User;
        let otel_cfg = ToolDecisionSource::Config;

        // 1) Approval
        let mut already_approved = false;

        let requirement = tool.exec_approval_requirement(req).unwrap_or_else(|| {
            default_exec_approval_requirement(approval_policy, &turn_ctx.sandbox_policy)
        });
        match requirement {
            ExecApprovalRequirement::Skip { .. } => {
                otel.tool_decision(otel_tn, otel_ci, &ReviewDecision::Approved, otel_cfg);
            }
            ExecApprovalRequirement::Forbidden { reason } => {
                return Err(ToolError::Rejected(reason));
            }
            ExecApprovalRequirement::NeedsApproval { reason, .. } => {
                let approval_ctx = ApprovalCtx {
                    session: tool_ctx.session,
                    turn: turn_ctx,
                    call_id: &tool_ctx.call_id,
                    retry_reason: reason,
                    network_approval_context: None,
                };
                let decision = tool.start_approval_async(req, approval_ctx).await;

                otel.tool_decision(otel_tn, otel_ci, &decision, otel_user.clone());

                match decision {
                    ReviewDecision::Denied | ReviewDecision::Abort => {
                        return Err(ToolError::Rejected("rejected by user".to_string()));
                    }
                    ReviewDecision::Approved
                    | ReviewDecision::ApprovedExecpolicyAmendment { .. }
                    | ReviewDecision::ApprovedForSession => {}
                }
                already_approved = true;
            }
        }

        // 2) First attempt under the selected sandbox.
        let has_managed_network_requirements = turn_ctx
            .config
            .config_layer_stack
            .requirements_toml()
            .network
            .is_some();
        let initial_sandbox = match tool.sandbox_mode_for_first_attempt(req) {
            SandboxOverride::BypassSandboxFirstAttempt => crate::exec::SandboxType::None,
            SandboxOverride::NoOverride => self.sandbox.select_initial(
                &turn_ctx.sandbox_policy,
                tool.sandbox_preference(),
                turn_ctx.windows_sandbox_level,
                has_managed_network_requirements,
            ),
        };

        // Platform-specific flag gating is handled by SandboxManager::select_initial
        // via crate::safety::get_platform_sandbox(..).
        let use_linux_sandbox_bwrap = turn_ctx.features.enabled(Feature::UseLinuxSandboxBwrap);
        let initial_attempt = SandboxAttempt {
            sandbox: initial_sandbox,
            policy: &turn_ctx.sandbox_policy,
            enforce_managed_network: has_managed_network_requirements,
            manager: &self.sandbox,
            sandbox_cwd: &turn_ctx.cwd,
            codex_linux_sandbox_exe: turn_ctx.codex_linux_sandbox_exe.as_ref(),
            use_linux_sandbox_bwrap,
            windows_sandbox_level: turn_ctx.windows_sandbox_level,
        };

        let (first_result, first_deferred_network_approval) = Self::run_attempt(
            tool,
            req,
            tool_ctx,
            &initial_attempt,
            has_managed_network_requirements,
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
                let network_approval_context = if has_managed_network_requirements {
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
                // Under `Never` or `OnRequest`, do not retry without sandbox; surface a concise
                // sandbox denial that preserves the original output.
                if !tool.wants_no_sandbox_approval(approval_policy) {
                    let allow_on_request_network_prompt =
                        matches!(approval_policy, AskForApproval::OnRequest)
                            && network_approval_context.is_some()
                            && matches!(
                                default_exec_approval_requirement(
                                    approval_policy,
                                    &turn_ctx.sandbox_policy
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
                    let approval_ctx = ApprovalCtx {
                        session: tool_ctx.session,
                        turn: turn_ctx,
                        call_id: &tool_ctx.call_id,
                        retry_reason: Some(retry_reason),
                        network_approval_context: network_approval_context.clone(),
                    };

                    let decision = tool.start_approval_async(req, approval_ctx).await;
                    otel.tool_decision(otel_tn, otel_ci, &decision, otel_user);

                    match decision {
                        ReviewDecision::Denied | ReviewDecision::Abort => {
                            return Err(ToolError::Rejected("rejected by user".to_string()));
                        }
                        ReviewDecision::Approved
                        | ReviewDecision::ApprovedExecpolicyAmendment { .. }
                        | ReviewDecision::ApprovedForSession => {}
                    }
                }

                let escalated_attempt = SandboxAttempt {
                    sandbox: crate::exec::SandboxType::None,
                    policy: &turn_ctx.sandbox_policy,
                    enforce_managed_network: has_managed_network_requirements,
                    manager: &self.sandbox,
                    sandbox_cwd: &turn_ctx.cwd,
                    codex_linux_sandbox_exe: None,
                    use_linux_sandbox_bwrap,
                    windows_sandbox_level: turn_ctx.windows_sandbox_level,
                };

                // Second attempt.
                let (retry_result, retry_deferred_network_approval) = Self::run_attempt(
                    tool,
                    req,
                    tool_ctx,
                    &escalated_attempt,
                    has_managed_network_requirements,
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
}

fn build_denial_reason_from_output(_output: &ExecToolCallOutput) -> String {
    // Keep approval reason terse and stable for UX/tests, but accept the
    // output so we can evolve heuristics later without touching call sites.
    "command failed; retry without sandbox?".to_string()
}
