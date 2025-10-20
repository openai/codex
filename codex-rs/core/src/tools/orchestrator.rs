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
                sub_id: &tool_ctx.sub_id,
                call_id: &tool_ctx.call_id,
                retry_reason: None,
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
        let initial_sandbox = self
            .sandbox
            .select_initial(&turn_ctx.sandbox_policy, tool.sandbox_preference());
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

                // Ask for approval before retrying without sandbox.
                let retry_reason = sandbox_denial_reason(initial_attempt.sandbox, output.as_ref());
                if !tool.should_bypass_approval(approval_policy, already_approved) {
                    let approval_ctx = ApprovalCtx {
                        session: tool_ctx.session,
                        sub_id: &tool_ctx.sub_id,
                        call_id: &tool_ctx.call_id,
                        retry_reason: Some(retry_reason.clone()),
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

fn sandbox_denial_reason(sandbox: crate::exec::SandboxType, output: &ExecToolCallOutput) -> String {
    let err = CodexErr::Sandbox(SandboxErr::Denied {
        output: Box::new(clone_exec_output(output)),
    });
    let message = get_error_message_ui(&err);
    format!("{sandbox:?} sandbox denied the command. {message}\nRetry without sandbox?")
}

fn clone_exec_output(output: &ExecToolCallOutput) -> ExecToolCallOutput {
    ExecToolCallOutput {
        exit_code: output.exit_code,
        stdout: clone_stream(&output.stdout),
        stderr: clone_stream(&output.stderr),
        aggregated_output: clone_stream(&output.aggregated_output),
        duration: output.duration,
        timed_out: output.timed_out,
    }
}

fn clone_stream(stream: &crate::exec::StreamOutput<String>) -> crate::exec::StreamOutput<String> {
    crate::exec::StreamOutput {
        text: stream.text.clone(),
        truncated_after_lines: stream.truncated_after_lines,
    }
}
