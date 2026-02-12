/*
Runtime: shell

Executes shell requests under the orchestrator: asks for approval when needed,
builds a CommandSpec, and runs it under the current SandboxAttempt.
*/
use crate::command_canonicalization::canonicalize_command_for_approval;
use crate::exec::ExecToolCallOutput;
use crate::features::Feature;
use crate::powershell::prefix_powershell_script_with_utf8;
use crate::sandboxing::SandboxPermissions;
use crate::sandboxing::execute_env;
use crate::shell::ShellType;
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
use codex_network_proxy::NetworkProxy;
use codex_protocol::protocol::ReviewDecision;
use futures::future::BoxFuture;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ShellRequest {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub timeout_ms: Option<u64>,
    pub env: std::collections::HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub sandbox_permissions: SandboxPermissions,
    pub justification: Option<String>,
    pub exec_approval_requirement: ExecApprovalRequirement,
}

#[derive(Default)]
pub struct ShellRuntime;

#[derive(serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ApprovalKey {
    command: Vec<String>,
    cwd: PathBuf,
    sandbox_permissions: SandboxPermissions,
}

impl ShellRuntime {
    pub fn new() -> Self {
        Self
    }

    fn stdout_stream(ctx: &ToolCtx<'_>) -> Option<crate::exec::StdoutStream> {
        Some(crate::exec::StdoutStream {
            sub_id: ctx.turn.sub_id.clone(),
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

    fn approval_keys(&self, req: &ShellRequest) -> Vec<Self::ApprovalKey> {
        vec![ApprovalKey {
            command: canonicalize_command_for_approval(&req.command),
            cwd: req.cwd.clone(),
            sandbox_permissions: req.sandbox_permissions,
        }]
    }

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a ShellRequest,
        ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision> {
        let keys = self.approval_keys(req);
        let command = req.command.clone();
        let cwd = req.cwd.clone();
        let reason = ctx
            .retry_reason
            .clone()
            .or_else(|| req.justification.clone());
        let session = ctx.session;
        let turn = ctx.turn;
        let call_id = ctx.call_id.to_string();
        Box::pin(async move {
            with_cached_approval(&session.services, "shell", keys, move || async move {
                session
                    .request_command_approval(
                        turn,
                        call_id,
                        command,
                        cwd,
                        reason,
                        ctx.network_approval_context.clone(),
                        req.exec_approval_requirement
                            .proposed_execpolicy_amendment()
                            .cloned(),
                    )
                    .await
            })
            .await
        })
    }

    fn exec_approval_requirement(&self, req: &ShellRequest) -> Option<ExecApprovalRequirement> {
        Some(req.exec_approval_requirement.clone())
    }

    fn sandbox_mode_for_first_attempt(&self, req: &ShellRequest) -> SandboxOverride {
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

impl ToolRuntime<ShellRequest, ExecToolCallOutput> for ShellRuntime {
    async fn run(
        &mut self,
        req: &ShellRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx<'_>,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let base_command = &req.command;
        let session_shell = ctx.session.user_shell();
        let command =
            maybe_wrap_shell_lc_with_snapshot(base_command, session_shell.as_ref(), &req.cwd);
        let command = if matches!(session_shell.shell_type, ShellType::PowerShell)
            && ctx.session.features().enabled(Feature::PowershellUtf8)
        {
            prefix_powershell_script_with_utf8(&command)
        } else {
            command
        };

        let spec = build_command_spec(
            &command,
            &req.cwd,
            &req.env,
            req.timeout_ms.into(),
            req.sandbox_permissions,
            req.justification.clone(),
        )?;
        let mut env = attempt
            .env_for(spec, req.network.as_ref())
            .map_err(|err| ToolError::Codex(err.into()))?;

        let network_attempt_id = req.network.as_ref().map(|_| Uuid::new_v4().to_string());
        if let Some(attempt_id) = network_attempt_id.as_ref() {
            ctx.session
                .register_network_approval_attempt(
                    attempt_id.clone(),
                    ctx.turn.sub_id.clone(),
                    ctx.call_id.clone(),
                    req.command.clone(),
                    req.cwd.clone(),
                )
                .await;
        }
        env.network_attempt_id = network_attempt_id.clone();

        let out = execute_env(env, attempt.policy, Self::stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex);

        if let Some(attempt_id) = network_attempt_id.as_deref() {
            ctx.session
                .unregister_network_approval_attempt(attempt_id)
                .await;
        }

        let out = out?;
        Ok(out)
    }
}
