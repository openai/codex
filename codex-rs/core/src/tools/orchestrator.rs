/*
Module: orchestrator

Central place for approvals + sandbox selection + retry semantics. Drives a
simple sequence for any ToolRuntime: approval → select sandbox → attempt →
retry without sandbox on denial (no re‑approval thanks to caching).
*/
use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::error::get_error_message_ui;
use crate::exec::ExecToolCallOutput;
use crate::sandboxing::SandboxManager;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ProvidesSandboxRetryData;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
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
        Rq: ProvidesSandboxRetryData,
    {
        let otel = turn_ctx.client.get_otel_event_manager();
        let otel_tn = &tool_ctx.tool_name;
        let otel_ci = &tool_ctx.call_id;
        let otel_user = codex_otel::otel_event_manager::ToolDecisionSource::User;
        let otel_cfg = codex_otel::otel_event_manager::ToolDecisionSource::Config;

        // 1) Approval
        let needs_initial_approval =
            tool.wants_initial_approval(req, approval_policy, &turn_ctx.sandbox_policy);
        let mut already_approved = false;

        if needs_initial_approval {
            let approval_ctx = ApprovalCtx {
                session: tool_ctx.session,
                turn: turn_ctx,
                call_id: &tool_ctx.call_id,
                retry_reason: None,
                risk: None,
            };
            let decision = tool.start_approval_async(req, approval_ctx).await;

            otel.tool_decision(otel_tn, otel_ci, decision, otel_user.clone());

            match decision {
                ReviewDecision::Denied | ReviewDecision::Abort => {
                    return Err(ToolError::Rejected("rejected by user".to_string()));
                }
                ReviewDecision::Approved | ReviewDecision::ApprovedForSession => {}
            }
            already_approved = true;
        } else {
            otel.tool_decision(otel_tn, otel_ci, ReviewDecision::Approved, otel_cfg);
        }

        // 2) First attempt under the selected sandbox.
        let mut initial_sandbox = self
            .sandbox
            .select_initial(&turn_ctx.sandbox_policy, tool.sandbox_preference());
        if tool.wants_escalated_first_attempt(req) {
            initial_sandbox = crate::exec::SandboxType::None;
        }
        let initial_attempt = SandboxAttempt {
            sandbox: initial_sandbox,
            policy: &turn_ctx.sandbox_policy,
            manager: &self.sandbox,
            sandbox_cwd: &turn_ctx.cwd,
            codex_linux_sandbox_exe: turn_ctx.codex_linux_sandbox_exe.as_ref(),
        };

        match tool.run(req, &initial_attempt, tool_ctx).await {
            Ok(out) => {
                // We have a successful initial result
                Ok(out)
            }
            Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied { output }))) => {
                if !tool.escalate_on_failure() {
                    return Err(ToolError::SandboxDenied(
                        "sandbox denied and no retry".to_string(),
                    ));
                }
                // Under `Never` or `OnRequest`, do not retry without sandbox; surface a concise message
                // derived from the actual output (platform-agnostic).
                if !tool.wants_no_sandbox_approval(approval_policy) {
                    let msg = build_never_denied_message_from_output(output.as_ref());
                    return Err(ToolError::SandboxDenied(msg));
                }

                // Ask for approval before retrying without sandbox.
                if !tool.should_bypass_approval(approval_policy, already_approved) {
                    let mut risk = None;

                    if let Some(metadata) = req.sandbox_retry_data() {
                        let err = SandboxErr::Denied {
                            output: output.clone(),
                        };
                        let friendly = get_error_message_ui(&CodexErr::Sandbox(err));
                        let failure_summary = format!("failed in sandbox: {friendly}");

                        risk = tool_ctx
                            .session
                            .assess_sandbox_command(
                                turn_ctx,
                                &tool_ctx.call_id,
                                &metadata.command,
                                Some(failure_summary.as_str()),
                            )
                            .await;
                    }

                    let reason_msg = build_denial_reason_from_output(output.as_ref());
                    let approval_ctx = ApprovalCtx {
                        session: tool_ctx.session,
                        turn: turn_ctx,
                        call_id: &tool_ctx.call_id,
                        retry_reason: Some(reason_msg),
                        risk,
                    };

                    let decision = tool.start_approval_async(req, approval_ctx).await;
                    otel.tool_decision(otel_tn, otel_ci, decision, otel_user);

                    match decision {
                        ReviewDecision::Denied | ReviewDecision::Abort => {
                            return Err(ToolError::Rejected("rejected by user".to_string()));
                        }
                        ReviewDecision::Approved | ReviewDecision::ApprovedForSession => {}
                    }
                }

                let escalated_attempt = SandboxAttempt {
                    sandbox: crate::exec::SandboxType::None,
                    policy: &turn_ctx.sandbox_policy,
                    manager: &self.sandbox,
                    sandbox_cwd: &turn_ctx.cwd,
                    codex_linux_sandbox_exe: None,
                };

                // Second attempt.
                (*tool).run(req, &escalated_attempt, tool_ctx).await
            }
            other => other,
        }
    }
}

fn build_never_denied_message_from_output(output: &ExecToolCallOutput) -> String {
    let body = format!(
        "{}\n{}\n{}",
        output.stderr.text, output.stdout.text, output.aggregated_output.text
    )
    .to_lowercase();

    let detail = if body.contains("permission denied") {
        Some("Permission denied")
    } else if body.contains("operation not permitted") {
        Some("Operation not permitted")
    } else if body.contains("read-only file system") {
        Some("Read-only file system")
    } else {
        None
    };

    match detail {
        Some(tag) => format!("failed in sandbox: {tag}"),
        None => "failed in sandbox".to_string(),
    }
}

fn build_denial_reason_from_output(_output: &ExecToolCallOutput) -> String {
    // Keep approval reason terse and stable for UX/tests, but accept the
    // output so we can evolve heuristics later without touching call sites.
    "command failed; retry without sandbox?".to_string()
}
