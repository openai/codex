/*
Runtime: shell

Executes shell requests under the orchestrator: asks for approval when needed,
builds a CommandSpec, and runs it under the current SandboxAttempt.
*/
use crate::approvals::Approvable;
use crate::approvals::ApprovalCtx;
use crate::approvals::ApprovalDecision;
use crate::exec::ExecToolCallOutput;
use crate::orchestrator::SandboxAttempt;
use crate::orchestrator::Sandboxable;
use crate::orchestrator::SandboxablePreference;
use crate::orchestrator::ToolCtx;
use crate::orchestrator::ToolError;
use crate::orchestrator::ToolRuntime;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::execute_env;
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

impl ShellRuntime {
    pub fn new() -> Self {
        Self
    }

    fn build_command_spec(req: &ShellRequest) -> Result<CommandSpec, ToolError> {
        let (program, args) = req
            .command
            .split_first()
            .ok_or_else(|| ToolError::Rejected("command args are empty".to_string()))?;
        Ok(CommandSpec {
            program: program.clone(),
            args: args.to_vec(),
            cwd: req.cwd.clone(),
            env: req.env.clone(),
            timeout_ms: req.timeout_ms,
            with_escalated_permissions: req.with_escalated_permissions,
            justification: req.justification.clone(),
        })
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

#[derive(serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ApprovalKey {
    command: Vec<String>,
    cwd: PathBuf,
    escalated: bool,
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

    fn reset_cache(&mut self) {}

    fn approval_preview(&self, req: &ShellRequest) -> Vec<String> {
        if req.command.is_empty() {
            return vec![];
        }
        vec![req.command.join(" ")]
    }

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a ShellRequest,
        ctx: ApprovalCtx<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ApprovalDecision> + Send + 'a>> {
        Box::pin(async move {
            let decision = ctx
                .session
                .request_command_approval(
                    ctx.sub_id.to_string(),
                    ctx.call_id.to_string(),
                    req.command.clone(),
                    req.cwd.clone(),
                    req.justification.clone(),
                )
                .await;
            ApprovalDecision::from(decision)
        })
    }
}

impl ToolRuntime<ShellRequest, ExecToolCallOutput> for ShellRuntime {
    async fn run(
        &mut self,
        req: &ShellRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx<'_>,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let spec = Self::build_command_spec(req)?;
        let env = attempt
            .env_for(&spec)
            .map_err(|err| ToolError::Codex(err.into()))?;
        let out = execute_env(&env, attempt.policy, Self::stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex)?;
        Ok(out)
    }
}
