/*
Module: orchestrator

Central place for approvals + sandbox selection + retry semantics. Drives a
simple sequence for any ToolRuntime: approval → select sandbox → attempt →
retry without sandbox on denial (no re‑approval thanks to caching).
*/
use crate::approvals::Approvable;
use crate::approvals::ApprovalCtx;
use crate::approvals::ApprovalDecision;
use crate::approvals::ApprovalStore;
use crate::codex::Session;
use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::error::get_error_message_ui;
use crate::exec::ExecToolCallOutput;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::SandboxManager;
use crate::sandboxing::SandboxTransformError;
use codex_protocol::protocol::AskForApproval;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SandboxablePreference {
    Auto,
    Require,
    Forbid,
}

pub(crate) trait Sandboxable {
    fn sandbox_preference(&self) -> SandboxablePreference;
    fn escalate_on_failure(&self) -> bool {
        true
    }
}

pub(crate) struct ToolCtx<'a> {
    pub session: &'a Session,
    pub sub_id: String,
    pub call_id: String,
}

#[derive(Debug)]
pub(crate) enum ToolError {
    Rejected(String),
    SandboxDenied(String),
    Codex(CodexErr),
}

pub(crate) trait ToolRuntime<Req, Out>: Approvable<Req> + Sandboxable {
    async fn run(
        &mut self,
        req: &Req,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
    ) -> Result<Out, ToolError>;
}

pub(crate) struct SandboxAttempt<'a> {
    pub sandbox: crate::exec::SandboxType,
    pub policy: &'a crate::protocol::SandboxPolicy,
    manager: &'a SandboxManager,
    sandbox_cwd: &'a Path,
    pub codex_linux_sandbox_exe: Option<&'a std::path::PathBuf>,
}

impl<'a> SandboxAttempt<'a> {
    pub fn env_for(
        &self,
        spec: &CommandSpec,
    ) -> Result<crate::sandboxing::ExecEnv, SandboxTransformError> {
        self.manager.transform(
            spec,
            self.policy,
            self.sandbox,
            self.sandbox_cwd,
            self.codex_linux_sandbox_exe,
        )
    }
}

pub(crate) struct ToolOrchestrator {
    approvals: ApprovalStore,
    sandbox: SandboxManager,
}

impl ToolOrchestrator {
    pub fn new() -> Self {
        Self {
            approvals: ApprovalStore::default(),
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
        // 1) Approval
        let key = tool.approval_key(req);
        let decision = match self.approvals.get(&key) {
            Some(d) => d,
            None => {
                if tool.should_bypass_approval(approval_policy) {
                    ApprovalDecision::Approved
                } else {
                    let ctx = ApprovalCtx {
                        session: tool_ctx.session,
                        sub_id: &tool_ctx.sub_id,
                        call_id: &tool_ctx.call_id,
                        retry_reason: None,
                    };
                    tool.start_approval_async(req, ctx).await
                }
            }
        };
        match decision {
            ApprovalDecision::Denied | ApprovalDecision::Abort => {
                return Err(ToolError::Rejected("rejected by user".to_string()));
            }
            ApprovalDecision::ApprovedForSession => self.approvals.put(key.clone(), decision),
            ApprovalDecision::Approved => {}
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
            },
            Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied { output }))) => {
                if !tool.escalate_on_failure() {
                    return Err(ToolError::SandboxDenied(
                        "sandbox denied and no retry".to_string(),
                    ));
                }

                // Ask for approval before retrying without sandbox.
                let retry_reason = sandbox_denial_reason(initial_attempt.sandbox, output.as_ref());
                if !tool.should_bypass_approval(approval_policy) {
                    let approval_ctx = ApprovalCtx {
                        session: tool_ctx.session,
                        sub_id: &tool_ctx.sub_id,
                        call_id: &tool_ctx.call_id,
                        retry_reason: Some(retry_reason.clone()),
                    };
                    match tool.start_approval_async(req, approval_ctx).await {
                        ApprovalDecision::Denied | ApprovalDecision::Abort => {
                            return Err(ToolError::Rejected("rejected by user".to_string()));
                        }
                        ApprovalDecision::ApprovedForSession => {
                            self.approvals
                                .put(key.clone(), ApprovalDecision::ApprovedForSession);
                        }
                        ApprovalDecision::Approved => {}
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
            other => other
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
