/*
Runtime: unified exec

Handles approval + sandbox orchestration for unified exec requests, delegating to
the session manager to spawn PTYs once an ExecEnv is prepared.
*/
use crate::error::CodexErr;
use crate::error::SandboxErr;
use crate::exec::ExecExpiration;
use crate::sandboxing::SandboxPermissions;
use crate::tools::runtimes::build_command_spec;
use crate::tools::runtimes::maybe_wrap_shell_lc_with_snapshot;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::SandboxOverride;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::SandboxablePreference;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::with_cached_approval;
use crate::unified_exec::UnifiedExecError;
use crate::unified_exec::UnifiedExecSession;
use crate::unified_exec::UnifiedExecSessionManager;
use codex_protocol::protocol::ReviewDecision;
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct UnifiedExecRequest {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub sandbox_permissions: SandboxPermissions,
    pub justification: Option<String>,
    pub exec_approval_requirement: ExecApprovalRequirement,
}

#[derive(serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct UnifiedExecApprovalKey {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub sandbox_permissions: SandboxPermissions,
}

pub struct UnifiedExecRuntime<'a> {
    manager: &'a UnifiedExecSessionManager,
}

impl UnifiedExecRequest {
    pub fn new(
        command: Vec<String>,
        cwd: PathBuf,
        env: HashMap<String, String>,
        sandbox_permissions: SandboxPermissions,
        justification: Option<String>,
        exec_approval_requirement: ExecApprovalRequirement,
    ) -> Self {
        Self {
            command,
            cwd,
            env,
            sandbox_permissions,
            justification,
            exec_approval_requirement,
        }
    }
}

impl<'a> UnifiedExecRuntime<'a> {
    pub fn new(manager: &'a UnifiedExecSessionManager) -> Self {
        Self { manager }
    }
}

impl Sandboxable for UnifiedExecRuntime<'_> {
    fn sandbox_preference(&self) -> SandboxablePreference {
        SandboxablePreference::Auto
    }

    fn escalate_on_failure(&self) -> bool {
        true
    }
}

impl Approvable<UnifiedExecRequest> for UnifiedExecRuntime<'_> {
    type ApprovalKey = UnifiedExecApprovalKey;

    fn approval_key(&self, req: &UnifiedExecRequest) -> Self::ApprovalKey {
        UnifiedExecApprovalKey {
            command: req.command.clone(),
            cwd: req.cwd.clone(),
            sandbox_permissions: req.sandbox_permissions,
        }
    }

    fn start_approval_async<'b>(
        &'b mut self,
        req: &'b UnifiedExecRequest,
        ctx: ApprovalCtx<'b>,
    ) -> BoxFuture<'b, ReviewDecision> {
        let key = self.approval_key(req);
        let session = ctx.session;
        let turn = ctx.turn;
        let call_id = ctx.call_id.to_string();
        let command = req.command.clone();
        let cwd = req.cwd.clone();
        let reason = ctx
            .retry_reason
            .clone()
            .or_else(|| req.justification.clone());
        Box::pin(async move {
            with_cached_approval(&session.services, key, || async move {
                session
                    .request_command_approval(
                        turn,
                        call_id,
                        command,
                        cwd,
                        reason,
                        req.exec_approval_requirement
                            .proposed_execpolicy_amendment()
                            .cloned(),
                    )
                    .await
            })
            .await
        })
    }

    fn exec_approval_requirement(
        &self,
        req: &UnifiedExecRequest,
    ) -> Option<ExecApprovalRequirement> {
        Some(req.exec_approval_requirement.clone())
    }

    fn sandbox_mode_for_first_attempt(&self, req: &UnifiedExecRequest) -> SandboxOverride {
        if req.sandbox_permissions.requires_escalated_permissions()
            || matches!(
                req.exec_approval_requirement,
                ExecApprovalRequirement::Skip {
                    bypass_sandbox: true,
                    ..
                }
            )
        {
            SandboxOverride::BypassSandboxFirstAttempt
        } else {
            SandboxOverride::NoOverride
        }
    }
}

impl<'a> ToolRuntime<UnifiedExecRequest, UnifiedExecSession> for UnifiedExecRuntime<'a> {
    async fn run(
        &mut self,
        req: &UnifiedExecRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx<'_>,
    ) -> Result<UnifiedExecSession, ToolError> {
        let base_command = &req.command;
        let session_shell = ctx.session.user_shell();
        let command = maybe_wrap_shell_lc_with_snapshot(base_command, session_shell.as_ref());

        let spec = build_command_spec(
            &command,
            &req.cwd,
            &req.env,
            ExecExpiration::DefaultTimeout,
            req.sandbox_permissions,
            req.justification.clone(),
        )
        .map_err(|_| ToolError::Rejected("missing command line for PTY".to_string()))?;
        let exec_env = attempt
            .env_for(spec)
            .map_err(|err| ToolError::Codex(err.into()))?;
        self.manager
            .open_session_with_exec_env(&exec_env)
            .await
            .map_err(|err| match err {
                UnifiedExecError::SandboxDenied { output, .. } => {
                    ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                        output: Box::new(output),
                    }))
                }
                other => ToolError::Rejected(other.to_string()),
            })
    }
}
