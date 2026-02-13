/*
Module: orchestrator

Central place for approvals + sandbox selection + retry semantics. Drives a
simple sequence for any ToolRuntime: approval → select sandbox → attempt →
retry with an escalated sandbox strategy on denial (no re‑approval thanks to
caching).
*/
use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::features::Feature;
use crate::network_policy_decision::NetworkPolicyDecisionPayload;
use crate::network_policy_decision::network_approval_context_from_payload;
use crate::sandboxing::SandboxManager;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::SandboxOverride;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::default_exec_approval_requirement;
use codex_otel::ToolDecisionSource;
use codex_protocol::approvals::NetworkApprovalContext;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;

pub(crate) struct ToolOrchestrator {
    sandbox: SandboxManager,
}

impl ToolOrchestrator {
    pub fn new() -> Self {
        Self {
            sandbox: SandboxManager::new(),
        }
    }

    pub async fn run<Rq, Out, T>(
        &mut self,
        tool: &mut T,
        req: &Rq,
        tool_ctx: &ToolCtx<'_>,
        turn_ctx: &crate::codex::TurnContext,
        approval_policy: AskForApproval,
    ) -> Result<Out, ToolError>
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

        match tool.run(req, &initial_attempt, tool_ctx).await {
            Ok(out) => {
                // We have a successful initial result
                Ok(out)
            }
            Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                output,
                network_policy_decision,
            }))) => {
                if let Some(payload) = network_policy_decision.as_ref() {
                    tracing::debug!(
                        "sandbox denied with structured network decision (decision={}, source={}, host={:?}, protocol={:?}, port={:?})",
                        payload.decision,
                        payload.source,
                        payload.host,
                        payload.protocol,
                        payload.port
                    );
                } else {
                    tracing::debug!("sandbox denied without structured network decision payload");
                }
                if network_policy_decision.is_some() {
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
                let retry_details = build_denial_reason(
                    network_policy_decision.as_ref(),
                    should_prompt_for_network_approval(turn_ctx),
                );
                tracing::debug!(
                    "retry denial reason prepared (reason={}, has_network_context={})",
                    retry_details.reason,
                    retry_details.network_approval_context.is_some()
                );

                // Most tools disallow no-sandbox retry under OnRequest. However, for managed
                // network denials with extracted network approval context, we still allow
                // prompting so the user can explicitly approve the specific host/protocol access.
                if !can_retry_without_sandbox(
                    tool.wants_no_sandbox_approval(approval_policy),
                    approval_policy,
                    &turn_ctx.sandbox_policy,
                    retry_details.network_approval_context.is_some(),
                ) {
                    tracing::debug!(
                        "retry without sandbox blocked by approval gate (approval_policy={:?}, has_network_context={})",
                        approval_policy,
                        retry_details.network_approval_context.is_some()
                    );
                    return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                        output,
                        network_policy_decision,
                    })));
                }

                // Ask for approval before retrying with the escalated sandbox.
                let should_bypass_retry_approval = should_bypass_retry_approval(
                    tool.should_bypass_approval(approval_policy, already_approved),
                    retry_details.network_approval_context.is_some(),
                );
                if !should_bypass_retry_approval {
                    let approval_ctx = ApprovalCtx {
                        session: tool_ctx.session,
                        turn: turn_ctx,
                        call_id: &tool_ctx.call_id,
                        retry_reason: Some(retry_details.reason),
                        network_approval_context: retry_details.network_approval_context.clone(),
                    };
                    tracing::debug!(
                        "requesting retry approval (reason={}, has_network_context={})",
                        approval_ctx.retry_reason.as_deref().unwrap_or("<none>"),
                        approval_ctx.network_approval_context.is_some()
                    );

                    let decision = tool.start_approval_async(req, approval_ctx).await;
                    otel.tool_decision(otel_tn, otel_ci, &decision, otel_user);
                    tracing::debug!("retry approval decision: {decision:?}");

                    match decision {
                        ReviewDecision::Denied | ReviewDecision::Abort => {
                            return Err(ToolError::Rejected("rejected by user".to_string()));
                        }
                        ReviewDecision::Approved
                        | ReviewDecision::ApprovedExecpolicyAmendment { .. }
                        | ReviewDecision::ApprovedForSession => {}
                    }
                }

                let temporary_allowed_host = if let Some(network_approval_context) =
                    retry_details.network_approval_context.as_ref()
                {
                    if let Some(network) = turn_ctx.network.as_ref() {
                        let granted_host = network
                            .grant_temporary_allowed_host(&network_approval_context.host)
                            .await;
                        if granted_host.is_none() {
                            tracing::warn!(
                                host = %network_approval_context.host,
                                "failed to grant temporary network host allowance; retry may remain blocked"
                            );
                        } else {
                            tracing::debug!(
                                "granted temporary network host allowance for retry: {}",
                                network_approval_context.host
                            );
                        }
                        granted_host.map(|host| (network.clone(), host))
                    } else {
                        tracing::warn!(
                            host = %network_approval_context.host,
                            "network approval context is present but no managed network proxy is available"
                        );
                        None
                    }
                } else {
                    None
                };

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
                let second_attempt = (*tool).run(req, &escalated_attempt, tool_ctx).await;
                if let Some((network, host)) = temporary_allowed_host {
                    network.revoke_temporary_allowed_host(&host).await;
                    tracing::debug!("revoked temporary network host allowance after retry: {host}");
                }
                second_attempt
            }
            other => other,
        }
    }
}

#[derive(Debug)]
struct RetryApprovalDetails {
    reason: String,
    network_approval_context: Option<NetworkApprovalContext>,
}

fn build_denial_reason(
    network_policy_decision: Option<&NetworkPolicyDecisionPayload>,
    network_prompting_enabled: bool,
) -> RetryApprovalDetails {
    let network_approval_context = if network_prompting_enabled {
        network_policy_decision.and_then(network_approval_context_from_payload)
    } else {
        None
    };
    let reason = if let Some(network_approval_context) = network_approval_context.as_ref() {
        format!(
            "Network access to \"{}\" is blocked by policy.",
            network_approval_context.host
        )
    } else {
        // Keep approval reason terse and stable for UX/tests.
        "command failed; retry without sandbox?".to_string()
    };
    RetryApprovalDetails {
        reason,
        network_approval_context,
    }
}

fn should_prompt_for_network_approval(turn_ctx: &crate::codex::TurnContext) -> bool {
    // Network retry prompting should follow the active managed proxy at runtime, not only
    // requirements.toml. This keeps behavior consistent for default sandbox sessions that still
    // route through a managed proxy.
    turn_ctx.network.is_some()
}

fn can_retry_without_sandbox(
    tool_wants_no_sandbox_approval: bool,
    approval_policy: AskForApproval,
    sandbox_policy: &codex_protocol::protocol::SandboxPolicy,
    has_network_approval_context: bool,
) -> bool {
    if tool_wants_no_sandbox_approval {
        return true;
    }

    if !matches!(approval_policy, AskForApproval::OnRequest) || !has_network_approval_context {
        return false;
    }

    // Keep retry prompting aligned with command exec approvals for OnRequest:
    // only restricted sandbox modes (ReadOnly/WorkspaceWrite) should prompt.
    matches!(
        default_exec_approval_requirement(approval_policy, sandbox_policy),
        ExecApprovalRequirement::NeedsApproval { .. }
    )
}

fn should_bypass_retry_approval(
    tool_wants_to_bypass_approval: bool,
    has_network_approval_context: bool,
) -> bool {
    tool_wants_to_bypass_approval && !has_network_approval_context
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network_policy_decision::NetworkPolicyDecisionPayload;
    use codex_protocol::approvals::NetworkApprovalProtocol;
    use pretty_assertions::assert_eq;

    #[test]
    fn build_denial_reason_extracts_network_context_when_enabled() {
        let decision = NetworkPolicyDecisionPayload {
            decision: "ask".to_string(),
            source: "decider".to_string(),
            protocol: Some("https_connect".to_string()),
            host: Some("example.com".to_string()),
            reason: Some("not_allowed".to_string()),
            port: Some(443),
        };

        let details = build_denial_reason(Some(&decision), true);

        assert_eq!(
            details.network_approval_context,
            Some(NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: NetworkApprovalProtocol::Https,
            })
        );
        assert_eq!(
            details.reason,
            "Network access to \"example.com\" is blocked by policy."
        );
    }

    #[test]
    fn build_denial_reason_skips_network_context_when_disabled() {
        let decision = NetworkPolicyDecisionPayload {
            decision: "ask".to_string(),
            source: "decider".to_string(),
            protocol: Some("https_connect".to_string()),
            host: Some("example.com".to_string()),
            reason: Some("not_allowed".to_string()),
            port: Some(443),
        };

        let details = build_denial_reason(Some(&decision), false);

        assert_eq!(details.network_approval_context, None);
        assert_eq!(details.reason, "command failed; retry without sandbox?");
    }

    #[test]
    fn build_denial_reason_ignores_non_ask_payloads() {
        let decision = NetworkPolicyDecisionPayload {
            decision: "deny".to_string(),
            source: "decider".to_string(),
            protocol: Some("http".to_string()),
            host: Some("example.com".to_string()),
            reason: Some("not_allowed".to_string()),
            port: Some(80),
        };

        let details = build_denial_reason(Some(&decision), true);
        assert_eq!(details.network_approval_context, None);
    }

    #[test]
    fn build_denial_reason_ignores_non_decider_payloads() {
        let decision = NetworkPolicyDecisionPayload {
            decision: "ask".to_string(),
            source: "baseline_policy".to_string(),
            protocol: Some("http".to_string()),
            host: Some("example.com".to_string()),
            reason: Some("not_allowed".to_string()),
            port: Some(80),
        };

        let details = build_denial_reason(Some(&decision), true);
        assert_eq!(details.network_approval_context, None);
    }

    #[test]
    fn build_denial_reason_extracts_http_protocol() {
        let decision = NetworkPolicyDecisionPayload {
            decision: "ask".to_string(),
            source: "decider".to_string(),
            protocol: Some("http".to_string()),
            host: Some("example.com".to_string()),
            reason: Some("not_allowed".to_string()),
            port: Some(80),
        };

        assert_eq!(
            build_denial_reason(Some(&decision), true).network_approval_context,
            Some(NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: NetworkApprovalProtocol::Http,
            })
        );
    }

    #[test]
    fn can_retry_without_sandbox_respects_default_on_request_gate() {
        assert!(!can_retry_without_sandbox(
            false,
            AskForApproval::OnRequest,
            &codex_protocol::protocol::SandboxPolicy::ReadOnly {
                access: codex_protocol::protocol::ReadOnlyAccess::FullAccess,
            },
            false
        ));
    }

    #[test]
    fn can_retry_without_sandbox_allows_on_request_for_network_context() {
        assert!(can_retry_without_sandbox(
            false,
            AskForApproval::OnRequest,
            &codex_protocol::protocol::SandboxPolicy::ReadOnly {
                access: codex_protocol::protocol::ReadOnlyAccess::FullAccess,
            },
            true
        ));
    }

    #[test]
    fn can_retry_without_sandbox_blocks_on_request_for_network_context_in_danger_full_access() {
        assert!(!can_retry_without_sandbox(
            false,
            AskForApproval::OnRequest,
            &codex_protocol::protocol::SandboxPolicy::DangerFullAccess,
            true
        ));
    }

    #[test]
    fn can_retry_without_sandbox_still_blocks_never_without_tool_override() {
        assert!(!can_retry_without_sandbox(
            false,
            AskForApproval::Never,
            &codex_protocol::protocol::SandboxPolicy::ReadOnly {
                access: codex_protocol::protocol::ReadOnlyAccess::FullAccess,
            },
            true
        ));
    }

    #[test]
    fn can_retry_without_sandbox_honors_tool_override() {
        assert!(can_retry_without_sandbox(
            true,
            AskForApproval::OnRequest,
            &codex_protocol::protocol::SandboxPolicy::ReadOnly {
                access: codex_protocol::protocol::ReadOnlyAccess::FullAccess,
            },
            false
        ));
    }

    #[test]
    fn retry_approval_not_bypassed_when_network_context_present() {
        assert!(!should_bypass_retry_approval(true, true));
    }

    #[test]
    fn retry_approval_bypassed_without_network_context() {
        assert!(should_bypass_retry_approval(true, false));
    }
}
