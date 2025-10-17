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
                        policy: approval_policy,
                        session: tool_ctx.session,
                        sub_id: &tool_ctx.sub_id,
                        call_id: &tool_ctx.call_id,
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

        // 2) Select initial sandbox
        let initial = self
            .sandbox
            .select_initial(&turn_ctx.sandbox_policy, tool.sandbox_preference());

        let codex_linux_sandbox_exe = turn_ctx.codex_linux_sandbox_exe.clone();

        let attempt = SandboxAttempt {
            sandbox: initial,
            policy: &turn_ctx.sandbox_policy,
            manager: &self.sandbox,
            sandbox_cwd: &turn_ctx.cwd,
            codex_linux_sandbox_exe: codex_linux_sandbox_exe.as_ref(),
        };

        // 3) Attempt #1
        match tool.run(req, &attempt, tool_ctx).await {
            Ok(out) => return Ok(out),
            Err(ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied { .. }))) => {}
            Err(err) => return Err(err),
        }

        // 4) Retry without sandbox
        if tool.escalate_on_failure() {
            tool.reset_cache();
            let attempt2 = SandboxAttempt {
                sandbox: crate::exec::SandboxType::None,
                policy: &turn_ctx.sandbox_policy,
                manager: &self.sandbox,
                sandbox_cwd: &turn_ctx.cwd,
                codex_linux_sandbox_exe: None,
            };
            return tool.run(req, &attempt2, tool_ctx).await;
        }

        Err(ToolError::SandboxDenied(
            "sandbox denied and no retry".to_string(),
        ))
    }
}
