/*
Runtime: unified exec

Handles approval + sandbox orchestration for unified exec requests, delegating to
the process manager to spawn PTYs once an ExecRequest is prepared.
*/
use crate::command_canonicalization::canonicalize_command_for_approval;
use crate::exec::ExecCapturePolicy;
use crate::exec::ExecExpiration;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::review_approval_request;
use crate::sandboxing::ExecOptions;
use crate::sandboxing::SandboxPermissions;
use crate::shell::ShellType;
use crate::tools::network_approval::NetworkApprovalMode;
use crate::tools::network_approval::NetworkApprovalSpec;
use crate::tools::runtimes::build_sandbox_command;
use crate::tools::runtimes::maybe_wrap_shell_lc_with_snapshot;
use crate::tools::runtimes::shell::zsh_fork_backend;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::PermissionRequestPayload;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::SandboxOverride;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::sandbox_override_for_first_attempt;
use crate::tools::sandboxing::with_cached_approval;
use crate::unified_exec::NoopSpawnLifecycle;
use crate::unified_exec::UnifiedExecError;
use crate::unified_exec::UnifiedExecProcess;
use crate::unified_exec::UnifiedExecProcessManager;
use codex_network_proxy::NetworkProxy;
use codex_protocol::error::CodexErr;
use codex_protocol::error::SandboxErr;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::ReviewDecision;
use codex_sandboxing::SandboxablePreference;
use codex_shell_command::powershell::prefix_powershell_script_with_utf8;
use codex_tools::UnifiedExecShellMode;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::future::BoxFuture;
use std::collections::HashMap;

/// Request payload used by the unified-exec runtime after approvals and
/// sandbox preferences have been resolved for the current turn.
#[derive(Clone, Debug)]
pub struct UnifiedExecRequest {
    pub command: Vec<String>,
    pub hook_command: String,
    pub process_id: i32,
    pub cwd: AbsolutePathBuf,
    pub env: HashMap<String, String>,
    pub explicit_env_overrides: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub tty: bool,
    pub sandbox_permissions: SandboxPermissions,
    pub additional_permissions: Option<PermissionProfile>,
    #[cfg(unix)]
    pub additional_permissions_preapproved: bool,
    pub justification: Option<String>,
    pub exec_approval_requirement: ExecApprovalRequirement,
}

/// Cache key for approval decisions that can be reused across equivalent
/// unified-exec launches.
#[derive(serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct UnifiedExecApprovalKey {
    pub command: Vec<String>,
    pub cwd: AbsolutePathBuf,
    pub tty: bool,
    pub sandbox_permissions: SandboxPermissions,
    pub additional_permissions: Option<PermissionProfile>,
}

/// Runtime adapter that keeps policy and sandbox orchestration on the
/// unified-exec side while delegating process startup to the manager.
pub struct UnifiedExecRuntime<'a> {
    manager: &'a UnifiedExecProcessManager,
    shell_mode: UnifiedExecShellMode,
}

impl<'a> UnifiedExecRuntime<'a> {
    /// Creates a runtime bound to the shared unified-exec process manager.
    pub fn new(manager: &'a UnifiedExecProcessManager, shell_mode: UnifiedExecShellMode) -> Self {
        Self {
            manager,
            shell_mode,
        }
    }

    fn guardian_request(
        req: &UnifiedExecRequest,
        ctx: &ApprovalCtx<'_>,
    ) -> GuardianApprovalRequest {
        GuardianApprovalRequest::ExecCommand {
            id: ctx.call_id.to_string(),
            command: req.command.clone(),
            cwd: req.cwd.to_path_buf(),
            sandbox_permissions: req.sandbox_permissions,
            additional_permissions: req.additional_permissions.clone(),
            justification: req.justification.clone(),
            tty: req.tty,
        }
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

    fn approval_keys(&self, req: &UnifiedExecRequest) -> Vec<Self::ApprovalKey> {
        vec![UnifiedExecApprovalKey {
            command: canonicalize_command_for_approval(&req.command),
            cwd: req.cwd.clone(),
            tty: req.tty,
            sandbox_permissions: req.sandbox_permissions,
            additional_permissions: req.additional_permissions.clone(),
        }]
    }

    fn start_approval_async<'b>(
        &'b mut self,
        req: &'b UnifiedExecRequest,
        ctx: ApprovalCtx<'b>,
    ) -> BoxFuture<'b, ReviewDecision> {
        let keys = self.approval_keys(req);
        let guardian_request = Self::guardian_request(req, &ctx);
        let session = ctx.session;
        let turn = ctx.turn;
        let call_id = ctx.call_id.to_string();
        let command = req.command.clone();
        let cwd = req.cwd.to_path_buf();
        let retry_reason = ctx.retry_reason.clone();
        let reason = retry_reason.clone().or_else(|| req.justification.clone());
        let guardian_review_id = ctx.guardian_review_id.clone();
        Box::pin(async move {
            if let Some(review_id) = guardian_review_id {
                return review_approval_request(
                    session,
                    turn,
                    review_id,
                    guardian_request,
                    retry_reason,
                )
                .await;
            }
            with_cached_approval(&session.services, "unified_exec", keys, || async move {
                let available_decisions = None;
                session
                    .request_command_approval(
                        turn,
                        call_id,
                        /*approval_id*/ None,
                        command,
                        cwd,
                        reason,
                        ctx.network_approval_context.clone(),
                        req.exec_approval_requirement
                            .proposed_execpolicy_amendment()
                            .cloned(),
                        req.additional_permissions.clone(),
                        available_decisions,
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

    fn permission_request_payload(
        &self,
        req: &UnifiedExecRequest,
    ) -> Option<PermissionRequestPayload> {
        Some(PermissionRequestPayload::bash(
            req.hook_command.clone(),
            req.sandbox_permissions,
            req.additional_permissions.clone(),
            req.justification.clone(),
        ))
    }

    fn guardian_approval_request(
        &self,
        req: &UnifiedExecRequest,
        ctx: &ApprovalCtx<'_>,
    ) -> Option<GuardianApprovalRequest> {
        Some(Self::guardian_request(req, ctx))
    }

    fn sandbox_mode_for_first_attempt(&self, req: &UnifiedExecRequest) -> SandboxOverride {
        sandbox_override_for_first_attempt(req.sandbox_permissions, &req.exec_approval_requirement)
    }
}

impl<'a> ToolRuntime<UnifiedExecRequest, UnifiedExecProcess> for UnifiedExecRuntime<'a> {
    fn network_approval_spec(
        &self,
        req: &UnifiedExecRequest,
        _ctx: &ToolCtx,
    ) -> Option<NetworkApprovalSpec> {
        req.network.as_ref()?;
        Some(NetworkApprovalSpec {
            network: req.network.clone(),
            mode: NetworkApprovalMode::Deferred,
        })
    }

    async fn run(
        &mut self,
        req: &UnifiedExecRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
    ) -> Result<UnifiedExecProcess, ToolError> {
        let base_command = &req.command;
        let session_shell = ctx.session.user_shell();
        let environment_is_remote = ctx
            .turn
            .environment
            .as_ref()
            .is_some_and(|environment| environment.is_remote());
        let command = if environment_is_remote {
            base_command.to_vec()
        } else {
            maybe_wrap_shell_lc_with_snapshot(
                base_command,
                session_shell.as_ref(),
                &req.cwd,
                &req.explicit_env_overrides,
                &req.env,
            )
        };
        let command = if matches!(session_shell.shell_type, ShellType::PowerShell) {
            prefix_powershell_script_with_utf8(&command)
        } else {
            command
        };

        let mut env = req.env.clone();
        if let Some(network) = req.network.as_ref() {
            network.apply_to_env(&mut env);
        }
        if let UnifiedExecShellMode::ZshFork(zsh_fork_config) = &self.shell_mode {
            let command =
                build_sandbox_command(&command, &req.cwd, &env, req.additional_permissions.clone())
                    .map_err(|_| ToolError::Rejected("missing command line for PTY".to_string()))?;
            let options = ExecOptions {
                expiration: ExecExpiration::DefaultTimeout,
                capture_policy: ExecCapturePolicy::ShellTool,
            };
            let exec_env = attempt
                .env_for(command, options, req.network.as_ref())
                .map_err(|err| ToolError::Codex(err.into()))?;
            match zsh_fork_backend::maybe_prepare_unified_exec(
                req,
                attempt,
                ctx,
                exec_env,
                zsh_fork_config,
            )
            .await?
            {
                Some(prepared) => {
                    let Some(environment) = ctx.turn.environment.as_ref() else {
                        return Err(ToolError::Rejected(
                            "exec_command is unavailable in this session".to_string(),
                        ));
                    };
                    if environment.is_remote() {
                        return Err(ToolError::Rejected(
                            "unified_exec zsh-fork is not supported when exec_server_url is configured".to_string(),
                        ));
                    }
                    return self
                        .manager
                        .open_session_with_exec_env(
                            req.process_id,
                            &prepared.exec_request,
                            req.tty,
                            prepared.spawn_lifecycle,
                            environment.as_ref(),
                        )
                        .await
                        .map_err(|err| match err {
                            UnifiedExecError::SandboxDenied { output, .. } => {
                                ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                                    output: Box::new(output),
                                    network_policy_decision: None,
                                }))
                            }
                            other => ToolError::Rejected(other.to_string()),
                        });
                }
                None => {
                    tracing::warn!(
                        "UnifiedExec ZshFork backend specified, but conditions for using it were not met, falling back to direct execution",
                    );
                }
            }
        }
        let command =
            build_sandbox_command(&command, &req.cwd, &env, req.additional_permissions.clone())
                .map_err(|_| ToolError::Rejected("missing command line for PTY".to_string()))?;
        let options = ExecOptions {
            expiration: ExecExpiration::DefaultTimeout,
            capture_policy: ExecCapturePolicy::ShellTool,
        };
        let exec_env = attempt
            .env_for(command, options, req.network.as_ref())
            .map_err(|err| ToolError::Codex(err.into()))?;
        let Some(environment) = ctx.turn.environment.as_ref() else {
            return Err(ToolError::Rejected(
                "exec_command is unavailable in this session".to_string(),
            ));
        };
        self.manager
            .open_session_with_exec_env(
                req.process_id,
                &exec_env,
                req.tty,
                Box::new(NoopSpawnLifecycle),
                environment.as_ref(),
            )
            .await
            .map_err(|err| match err {
                UnifiedExecError::SandboxDenied { output, .. } => {
                    ToolError::Codex(CodexErr::Sandbox(SandboxErr::Denied {
                        output: Box::new(output),
                        network_policy_decision: None,
                    }))
                }
                other => ToolError::Rejected(other.to_string()),
            })
    }
}
