/*
Module: orchestrator

Central place for approvals + sandbox selection + retry semantics. Drives a
simple sequence for any ToolRuntime: approval → select sandbox → attempt →
retry with an escalated sandbox strategy on denial (no re‑approval thanks to
caching).
*/
use crate::guardian::GuardianApprovalReview;
use crate::guardian::GuardianApprovalReviewResult;
use crate::guardian::GuardianApprovalReviewStatus;
use crate::guardian::GuardianAssessmentOutcome;
use crate::guardian::guardian_rejection_message;
use crate::guardian::new_guardian_review_id;
use crate::guardian::review_approval_request_with_review;
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
use codex_hooks::PermissionRequestGuardianReview;
use codex_hooks::PermissionRequestGuardianReviewDecision;
use codex_hooks::PermissionRequestGuardianReviewStatus as HookGuardianReviewStatus;
use codex_otel::SessionTelemetry;
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
        has_managed_network_requirements: bool,
    ) -> (Result<Out, ToolError>, Option<DeferredNetworkApproval>)
    where
        T: ToolRuntime<Rq, Out>,
    {
        let network_approval = begin_network_approval(
            &tool_ctx.session,
            &tool_ctx.turn.sub_id,
            has_managed_network_requirements,
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
        turn_ctx: &crate::codex::TurnContext,
        approval_policy: AskForApproval,
    ) -> Result<OrchestratorRunResult<Out>, ToolError>
    where
        T: ToolRuntime<Rq, Out>,
    {
        let otel = turn_ctx.session_telemetry.clone();
        let otel_tn = &tool_ctx.tool_name;
        let otel_ci = &tool_ctx.call_id;
        let use_guardian = routes_approval_to_guardian(turn_ctx);
        // 1) Approval
        let mut already_approved = false;

        let requirement = tool.exec_approval_requirement(req).unwrap_or_else(|| {
            default_exec_approval_requirement(approval_policy, &turn_ctx.file_system_sandbox_policy)
        });
        match requirement {
            ExecApprovalRequirement::Skip { .. } => {
                otel.tool_decision(
                    otel_tn,
                    otel_ci,
                    &ReviewDecision::Approved,
                    ToolDecisionSource::Config,
                );
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
                    approval_ctx,
                    turn_ctx,
                    &otel,
                    otel_tn,
                    otel_ci,
                )
                .await?;

                match decision {
                    ReviewDecision::Denied | ReviewDecision::Abort => {
                        let reason = if let Some(review_id) = guardian_review_id.as_deref() {
                            guardian_rejection_message(tool_ctx.session.as_ref(), review_id).await
                        } else {
                            "rejected by user".to_string()
                        };
                        return Err(ToolError::Rejected(reason));
                    }
                    ReviewDecision::Approved
                    | ReviewDecision::ApprovedExecpolicyAmendment { .. }
                    | ReviewDecision::ApprovedForSession => {}
                    ReviewDecision::NetworkPolicyAmendment {
                        network_policy_amendment,
                    } => match network_policy_amendment.action {
                        NetworkPolicyRuleAction::Allow => {}
                        NetworkPolicyRuleAction::Deny => {
                            return Err(ToolError::Rejected("rejected by user".to_string()));
                        }
                    },
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
            SandboxOverride::BypassSandboxFirstAttempt => SandboxType::None,
            SandboxOverride::NoOverride => self.sandbox.select_initial(
                &turn_ctx.file_system_sandbox_policy,
                turn_ctx.network_sandbox_policy,
                tool.sandbox_preference(),
                turn_ctx.windows_sandbox_level,
                has_managed_network_requirements,
            ),
        };

        // Platform-specific flag gating is handled by SandboxManager::select_initial.
        let use_legacy_landlock = turn_ctx.features.use_legacy_landlock();
        let initial_attempt = SandboxAttempt {
            sandbox: initial_sandbox,
            policy: &turn_ctx.sandbox_policy,
            file_system_policy: &turn_ctx.file_system_sandbox_policy,
            network_policy: turn_ctx.network_sandbox_policy,
            enforce_managed_network: has_managed_network_requirements,
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
                    let decision = Self::request_approval(
                        tool,
                        req,
                        approval_ctx,
                        turn_ctx,
                        &otel,
                        otel_tn,
                        otel_ci,
                    )
                    .await?;

                    match decision {
                        ReviewDecision::Denied | ReviewDecision::Abort => {
                            let reason = if let Some(review_id) = guardian_review_id.as_deref() {
                                guardian_rejection_message(tool_ctx.session.as_ref(), review_id)
                                    .await
                            } else {
                                "rejected by user".to_string()
                            };
                            return Err(ToolError::Rejected(reason));
                        }
                        ReviewDecision::Approved
                        | ReviewDecision::ApprovedExecpolicyAmendment { .. }
                        | ReviewDecision::ApprovedForSession => {}
                        ReviewDecision::NetworkPolicyAmendment {
                            network_policy_amendment,
                        } => match network_policy_amendment.action {
                            NetworkPolicyRuleAction::Allow => {}
                            NetworkPolicyRuleAction::Deny => {
                                return Err(ToolError::Rejected("rejected by user".to_string()));
                            }
                        },
                    }
                }

                let escalated_attempt = SandboxAttempt {
                    sandbox: SandboxType::None,
                    policy: &turn_ctx.sandbox_policy,
                    file_system_policy: &turn_ctx.file_system_sandbox_policy,
                    network_policy: turn_ctx.network_sandbox_policy,
                    enforce_managed_network: has_managed_network_requirements,
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

    // Centralize one approval prompt for three possible decision makers. If
    // this prompt would normally route to guardian, run guardian first and pass
    // its result to PermissionRequest hooks as advisory context. The hook can
    // still answer the prompt; if it stays quiet, reuse the guardian decision
    // instead of asking guardian again. Without a hook or reusable guardian
    // result, fall back to the runtime's normal approval path.
    async fn request_approval<Rq, Out, T>(
        tool: &mut T,
        req: &Rq,
        approval_ctx: ApprovalCtx<'_>,
        turn_ctx: &crate::codex::TurnContext,
        otel: &SessionTelemetry,
        otel_tn: &str,
        otel_ci: &str,
    ) -> Result<ReviewDecision, ToolError>
    where
        T: ToolRuntime<Rq, Out>,
    {
        let guardian_review = if routes_approval_to_guardian(turn_ctx) {
            match tool.guardian_approval_request(req, &approval_ctx) {
                Some(request) => Some(
                    review_approval_request_with_review(
                        approval_ctx.session,
                        approval_ctx.turn,
                        approval_ctx
                            .guardian_review_id
                            .clone()
                            .expect("guardian review id should be present for guardian approvals"),
                        request,
                        approval_ctx.retry_reason.clone(),
                    )
                    .await,
                ),
                None => None,
            }
        } else {
            None
        };

        if let Some(permission_request) = tool.permission_request_payload(req) {
            match run_permission_request_hooks(
                approval_ctx.session,
                approval_ctx.turn,
                approval_ctx.call_id.to_string(),
                permission_request.tool_name,
                permission_request.command,
                permission_request.sandbox_permissions,
                permission_request.additional_permissions,
                permission_request.justification,
                guardian_review
                    .as_ref()
                    .map(|review| permission_request_guardian_review(review.review.clone())),
            )
            .await
            {
                Some(PermissionRequestDecision::Allow) => {
                    otel.tool_decision(
                        otel_tn,
                        otel_ci,
                        &ReviewDecision::Approved,
                        ToolDecisionSource::Config,
                    );
                    return Ok(ReviewDecision::Approved);
                }
                Some(PermissionRequestDecision::Deny { message }) => {
                    otel.tool_decision(
                        otel_tn,
                        otel_ci,
                        &ReviewDecision::Denied,
                        ToolDecisionSource::Config,
                    );
                    return Err(ToolError::Rejected(message));
                }
                None => {}
            }
        }

        if let Some(GuardianApprovalReviewResult { decision, .. }) = guardian_review {
            otel.tool_decision(
                otel_tn,
                otel_ci,
                &decision,
                ToolDecisionSource::AutomatedReviewer,
            );
            return Ok(decision);
        }

        let decision = tool.start_approval_async(req, approval_ctx).await;
        let otel_source = if routes_approval_to_guardian(turn_ctx) {
            ToolDecisionSource::AutomatedReviewer
        } else {
            ToolDecisionSource::User
        };
        otel.tool_decision(otel_tn, otel_ci, &decision, otel_source);
        Ok(decision)
    }
}

fn build_denial_reason_from_output(_output: &ExecToolCallOutput) -> String {
    // Keep approval reason terse and stable for UX/tests, but accept the
    // output so we can evolve heuristics later without touching call sites.
    "command failed; retry without sandbox?".to_string()
}

fn permission_request_guardian_review(
    review: GuardianApprovalReview,
) -> PermissionRequestGuardianReview {
    PermissionRequestGuardianReview {
        status: match review.status {
            GuardianApprovalReviewStatus::Approved => HookGuardianReviewStatus::Approved,
            GuardianApprovalReviewStatus::Denied => HookGuardianReviewStatus::Denied,
            GuardianApprovalReviewStatus::Aborted => HookGuardianReviewStatus::Aborted,
            GuardianApprovalReviewStatus::Failed => HookGuardianReviewStatus::Failed,
            GuardianApprovalReviewStatus::TimedOut => HookGuardianReviewStatus::TimedOut,
        },
        decision: review.decision.map(|decision| match decision {
            GuardianAssessmentOutcome::Allow => PermissionRequestGuardianReviewDecision::Allow,
            GuardianAssessmentOutcome::Deny => PermissionRequestGuardianReviewDecision::Deny,
        }),
        risk_level: review.risk_level,
        user_authorization: review.user_authorization,
        rationale: review.rationale,
    }
}
