/*
Runtime: shell

Executes shell requests under the orchestrator: asks for approval when needed,
builds a CommandSpec, and runs it under the current SandboxAttempt.
*/
use crate::command_safety::is_dangerous_command::command_might_be_dangerous;
use crate::command_safety::is_safe_command::is_known_safe_command;
use crate::exec::ExecToolCallOutput;
use crate::protocol::SandboxPolicy;
use crate::sandboxing::execute_env;
use crate::tools::runtimes::build_command_spec;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::SandboxablePreference;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::with_cached_approval;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use futures::future::BoxFuture;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ShellRequest {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub timeout_ms: Option<u64>,
    pub env: std::collections::HashMap<String, String>,
    pub with_escalated_permissions: Option<bool>,
    pub justification: Option<String>,
}

#[derive(Default)]
pub struct ShellRuntime;

#[derive(serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ApprovalKey {
    command: Vec<String>,
    cwd: PathBuf,
    escalated: bool,
}

impl ShellRuntime {
    pub fn new() -> Self {
        Self
    }

    fn stdout_stream(ctx: &ToolCtx<'_>) -> Option<crate::exec::StdoutStream> {
        Some(crate::exec::StdoutStream {
            sub_id: ctx.sub_id.clone(),
            call_id: ctx.call_id.clone(),
            tx_event: ctx.session.get_tx_event(),
        })
    }
}

impl Sandboxable for ShellRuntime {
    fn sandbox_preference(&self) -> SandboxablePreference {
        SandboxablePreference::Auto
    }
    fn escalate_on_failure(&self) -> bool {
        true
    }
}

impl Approvable<ShellRequest> for ShellRuntime {
    type ApprovalKey = ApprovalKey;

    fn approval_key(&self, req: &ShellRequest) -> Self::ApprovalKey {
        ApprovalKey {
            command: req.command.clone(),
            cwd: req.cwd.clone(),
            escalated: req.with_escalated_permissions.unwrap_or(false),
        }
    }

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a ShellRequest,
        ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision> {
        let key = self.approval_key(req);
        let command = req.command.clone();
        let cwd = req.cwd.clone();
        let reason = ctx
            .retry_reason
            .clone()
            .or_else(|| req.justification.clone());
        let session = ctx.session;
        let sub_id = ctx.sub_id.to_string();
        let call_id = ctx.call_id.to_string();
        Box::pin(async move {
            with_cached_approval(&session.services, key, || async move {
                session
                    .request_command_approval(sub_id, call_id, command, cwd, reason)
                    .await
            })
            .await
        })
    }

    fn wants_initial_approval(
        &self,
        req: &ShellRequest,
        policy: AskForApproval,
        sandbox_policy: &SandboxPolicy,
    ) -> bool {
        if is_known_safe_command(&req.command) {
            return false;
        }
        match policy {
            AskForApproval::Never | AskForApproval::OnFailure => false,
            AskForApproval::OnRequest => {
                // In DangerFullAccess, only prompt if the command looks dangerous.
                if matches!(sandbox_policy, SandboxPolicy::DangerFullAccess) {
                    return command_might_be_dangerous(&req.command);
                }

                // In restricted sandboxes (ReadOnly/WorkspaceWrite), do not prompt for
                // non‑escalated, non‑dangerous commands — let the sandbox enforce
                // restrictions (e.g., block network/write) without a user prompt.
                let wants_escalation = req.with_escalated_permissions.unwrap_or(false);
                if wants_escalation {
                    return true;
                }
                command_might_be_dangerous(&req.command)
            }
            AskForApproval::UnlessTrusted => !is_known_safe_command(&req.command),
        }
    }

    fn wants_escalated_first_attempt(&self, req: &ShellRequest) -> bool {
        req.with_escalated_permissions.unwrap_or(false)
    }
}

impl ToolRuntime<ShellRequest, ExecToolCallOutput> for ShellRuntime {
    async fn run(
        &mut self,
        req: &ShellRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx<'_>,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let spec = build_command_spec(
            &req.command,
            &req.cwd,
            &req.env,
            req.timeout_ms,
            req.with_escalated_permissions,
            req.justification.clone(),
        )?;
        let env = attempt
            .env_for(&spec)
            .map_err(|err| ToolError::Codex(err.into()))?;
        let out = execute_env(&env, attempt.policy, Self::stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex)?;
        Ok(out)
    }
}
