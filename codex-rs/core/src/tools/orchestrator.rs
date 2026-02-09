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
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use serde::Deserialize;

pub(crate) struct ToolOrchestrator {
    sandbox: SandboxManager,
}

const NETWORK_POLICY_DECISION_PREFIX: &str = "CODEX_NETWORK_POLICY_DECISION ";

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
            Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied { output }))) => {
                if !tool.escalate_on_failure() {
                    return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                        output,
                    })));
                }
                // Under `Never` or `OnRequest`, do not retry without sandbox; surface a concise
                // sandbox denial that preserves the original output.
                if !tool.wants_no_sandbox_approval(approval_policy) {
                    return Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                        output,
                    })));
                }

                // Ask for approval before retrying with the escalated sandbox.
                if !tool.should_bypass_approval(approval_policy, already_approved) {
                    let retry_details = build_denial_reason_from_output(
                        output.as_ref(),
                        should_prompt_for_network_approval(turn_ctx),
                    );
                    let approval_ctx = ApprovalCtx {
                        session: tool_ctx.session,
                        turn: turn_ctx,
                        call_id: &tool_ctx.call_id,
                        retry_reason: Some(retry_details.reason),
                        network_approval_context: retry_details.network_approval_context,
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
                (*tool).run(req, &escalated_attempt, tool_ctx).await
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

fn build_denial_reason_from_output(
    output: &ExecToolCallOutput,
    network_prompting_enabled: bool,
) -> RetryApprovalDetails {
    let network_approval_context = if network_prompting_enabled {
        extract_network_approval_context(output)
    } else {
        None
    };
    let reason = if let Some(network_approval_context) = network_approval_context.as_ref() {
        format!(
            "Network access to \"{}\" is blocked by policy.",
            network_approval_context.host
        )
    } else {
        // Keep approval reason terse and stable for UX/tests, but accept the
        // output so we can evolve heuristics later without touching call sites.
        "command failed; retry without sandbox?".to_string()
    };
    RetryApprovalDetails {
        reason,
        network_approval_context,
    }
}

fn should_prompt_for_network_approval(turn_ctx: &crate::codex::TurnContext) -> bool {
    matches!(
        turn_ctx
            .config
            .config_layer_stack
            .requirements_toml()
            .network
            .as_ref()
            .and_then(|network| network.enabled),
        Some(true)
    )
}

fn extract_network_approval_context(output: &ExecToolCallOutput) -> Option<NetworkApprovalContext> {
    [
        output.stderr.text.as_str(),
        output.stdout.text.as_str(),
        output.aggregated_output.text.as_str(),
    ]
    .into_iter()
    .find_map(extract_network_approval_context_from_text)
}

fn extract_network_approval_context_from_text(text: &str) -> Option<NetworkApprovalContext> {
    text.lines()
        .find_map(|line| line.strip_prefix(NETWORK_POLICY_DECISION_PREFIX))
        .and_then(|payload| serde_json::from_str::<NetworkPolicyDecisionPayload>(payload).ok())
        .and_then(|payload| {
            if payload.decision != "ask" || payload.source != "decider" {
                return None;
            }

            let protocol = match payload.protocol.as_str() {
                "http" => NetworkApprovalProtocol::Http,
                "https" | "https_connect" => NetworkApprovalProtocol::Https,
                _ => return None,
            };

            let host = payload.host.trim();
            if host.is_empty() {
                return None;
            }

            Some(NetworkApprovalContext {
                host: host.to_string(),
                protocol,
            })
        })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NetworkPolicyDecisionPayload {
    decision: String,
    source: String,
    protocol: String,
    host: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::StreamOutput;
    use pretty_assertions::assert_eq;

    fn output_with_stderr(stderr: &str) -> ExecToolCallOutput {
        ExecToolCallOutput {
            stderr: StreamOutput::new(stderr.to_string()),
            ..ExecToolCallOutput::default()
        }
    }

    #[test]
    fn build_denial_reason_extracts_network_context_when_enabled() {
        let output = output_with_stderr(
            "CODEX_NETWORK_POLICY_DECISION {\"decision\":\"ask\",\"source\":\"decider\",\"protocol\":\"https_connect\",\"host\":\"example.com\",\"port\":443}\nblocked",
        );

        let details = build_denial_reason_from_output(&output, true);

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
        let output = output_with_stderr(
            "CODEX_NETWORK_POLICY_DECISION {\"decision\":\"ask\",\"source\":\"decider\",\"protocol\":\"https_connect\",\"host\":\"example.com\",\"port\":443}\nblocked",
        );

        let details = build_denial_reason_from_output(&output, false);

        assert_eq!(details.network_approval_context, None);
        assert_eq!(details.reason, "command failed; retry without sandbox?");
    }

    #[test]
    fn extract_network_approval_context_ignores_non_ask_payloads() {
        let text = "CODEX_NETWORK_POLICY_DECISION {\"decision\":\"deny\",\"source\":\"decider\",\"protocol\":\"http\",\"host\":\"example.com\",\"port\":80}";

        assert_eq!(extract_network_approval_context_from_text(text), None);
    }

    #[test]
    fn extract_network_approval_context_ignores_non_decider_payloads() {
        let text = "CODEX_NETWORK_POLICY_DECISION {\"decision\":\"ask\",\"source\":\"baseline_policy\",\"protocol\":\"http\",\"host\":\"example.com\",\"port\":80}";

        assert_eq!(extract_network_approval_context_from_text(text), None);
    }
}
