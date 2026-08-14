/*
Runtime: shell

Executes shell requests under the orchestrator: asks for approval when needed,
builds sandbox transform inputs, and runs them under the current SandboxAttempt.
*/
#[cfg(unix)]
pub(crate) mod unix_escalation;
pub(crate) mod zsh_fork_backend;

use crate::exec::ExecCapturePolicy;
use crate::guardian::GuardianNetworkAccessTrigger;
use crate::plugins::metrics::finish_and_track_measurements;
use crate::plugins::metrics::sidecar_for_command;
use crate::sandboxing::ExecOptions;
use crate::sandboxing::SandboxPermissions;
use crate::sandboxing::execute_env;
use crate::session::turn_context::TurnEnvironment;
use crate::shell::ShellType;
use crate::tools::flat_tool_name;
use crate::tools::network_approval::NetworkApprovalMode;
use crate::tools::network_approval::NetworkApprovalSpec;
use crate::tools::runtimes::RuntimePathPrepends;
#[cfg(unix)]
use crate::tools::runtimes::apply_zsh_fork_path_prepend;
use crate::tools::runtimes::build_sandbox_command;
use crate::tools::runtimes::disable_powershell_profile_for_elevated_windows_sandbox;
use crate::tools::runtimes::exec_env_for_sandbox_permissions;
use crate::tools::runtimes::maybe_wrap_shell_lc_with_snapshot;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalAction;
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::managed_network_for_sandbox_permissions;
use crate::tools::sandboxing::sandbox_permissions_preserving_denied_reads;
use codex_core_plugins::PluginMetricsSidecar;
use codex_network_proxy::NetworkProxy;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_sandboxing::SandboxablePreference;
use codex_sandboxing::policy_transforms::merge_permission_profiles;
use codex_shell_command::powershell::prefix_powershell_script_with_utf8;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
pub struct ShellRequest {
    pub command: Vec<String>,
    pub turn_environment: TurnEnvironment,
    pub shell_type: Option<ShellType>,
    pub hook_command: String,
    pub cwd: AbsolutePathBuf,
    pub timeout_ms: Option<u64>,
    pub cancellation_token: CancellationToken,
    pub env: HashMap<String, String>,
    pub explicit_env_overrides: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub sandbox_permissions: SandboxPermissions,
    pub additional_permissions: Option<AdditionalPermissionProfile>,
    #[cfg(unix)]
    pub additional_permissions_preapproved: bool,
    pub justification: Option<String>,
    pub exec_approval_requirement: ExecApprovalRequirement,
}

/// Selects `ShellRuntime` behavior for different callers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShellRuntimeBackend {
    /// Legacy backend for the `shell_command` tool.
    ///
    /// Keeps `shell_command` on the standard shell runtime flow without the
    /// zsh-fork shell-escalation adapter.
    ShellCommandClassic,
    /// zsh-fork backend for the `shell_command` tool.
    ///
    /// On Unix, attempts to run via the zsh-fork + `codex-shell-escalation`
    /// adapter, with fallback to the standard shell runtime flow if
    /// prerequisites are not met.
    ShellCommandZshFork,
}

pub struct ShellRuntime {
    backend: ShellRuntimeBackend,
}

#[derive(serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ApprovalKey {
    pub(crate) environment_id: String,
    pub(crate) command: Vec<String>,
    pub(crate) cwd: PathUri,
    pub(crate) sandbox_permissions: SandboxPermissions,
    pub(crate) additional_permissions: Option<AdditionalPermissionProfile>,
}

impl ShellRuntime {
    pub(crate) fn for_shell_command(backend: ShellRuntimeBackend) -> Self {
        Self { backend }
    }

    fn stdout_stream(ctx: &ToolCtx) -> Option<crate::exec::StdoutStream> {
        Some(crate::exec::StdoutStream {
            sub_id: ctx.step_context.turn.sub_id.clone(),
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
    fn approval_action(
        &self,
        req: &ShellRequest,
        call_id: &str,
    ) -> std::io::Result<ApprovalAction> {
        Ok(ApprovalAction::Shell {
            id: call_id.to_string(),
            environment_id: req.turn_environment.selection.environment_id.clone(),
            command: req.command.clone(),
            hook_command: req.hook_command.clone(),
            cwd: PathUri::from_abs_path(&req.cwd),
            sandbox_permissions: req.sandbox_permissions,
            additional_permissions: req.additional_permissions.clone(),
            justification: req.justification.clone(),
            proposed_execpolicy_amendment: req
                .exec_approval_requirement
                .proposed_execpolicy_amendment()
                .cloned(),
        })
    }

    fn exec_approval_requirement(&self, req: &ShellRequest) -> Option<ExecApprovalRequirement> {
        Some(req.exec_approval_requirement.clone())
    }

    fn sandbox_permissions(&self, req: &ShellRequest) -> SandboxPermissions {
        req.sandbox_permissions
    }
}

impl ToolRuntime<ShellRequest, ExecToolCallOutput> for ShellRuntime {
    fn turn_environment<'a>(&self, req: &'a ShellRequest) -> &'a TurnEnvironment {
        &req.turn_environment
    }

    fn network_approval_spec(
        &self,
        req: &ShellRequest,
        ctx: &ToolCtx,
    ) -> Option<NetworkApprovalSpec> {
        let file_system_sandbox_policy = req
            .turn_environment
            .permission_profile()
            .file_system_sandbox_policy();
        let sandbox_permissions = sandbox_permissions_preserving_denied_reads(
            req.sandbox_permissions,
            &file_system_sandbox_policy,
        );
        let network =
            managed_network_for_sandbox_permissions(req.network.as_ref(), sandbox_permissions)?;
        Some(NetworkApprovalSpec {
            network: Some(network.clone()),
            mode: NetworkApprovalMode::Immediate,
            trigger: GuardianNetworkAccessTrigger {
                call_id: ctx.call_id.clone(),
                tool_name: flat_tool_name(&ctx.tool_name).into_owned(),
                command: req.command.clone(),
                cwd: req.cwd.clone(),
                sandbox_permissions: req.sandbox_permissions,
                additional_permissions: req.additional_permissions.clone(),
                justification: req.justification.clone(),
                tty: None,
            },
            command: req.hook_command.clone(),
            environment_id: req.turn_environment.selection.environment_id.clone(),
            permission_profile: req.turn_environment.permission_profile().clone(),
        })
    }

    async fn run(
        &mut self,
        req: &ShellRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let session_shell = ctx.session.user_shell();
        let shell = req
            .turn_environment
            .shell
            .as_ref()
            .unwrap_or(session_shell.as_ref());
        let shell_snapshot_location = req.turn_environment.shell_snapshot(&req.cwd);
        let (file_system_sandbox_policy, _) = attempt.permissions.to_runtime_permissions();
        let sandbox_permissions = sandbox_permissions_preserving_denied_reads(
            req.sandbox_permissions,
            &file_system_sandbox_policy,
        );
        let managed_network =
            managed_network_for_sandbox_permissions(req.network.as_ref(), sandbox_permissions);
        let mut env = exec_env_for_sandbox_permissions(&req.env, sandbox_permissions);
        let explicit_env_overrides = req.explicit_env_overrides.clone();
        let cwd = PathUri::from_abs_path(&req.cwd);
        let metrics_sidecar = sidecar_for_command(
            ctx,
            &req.command,
            &cwd,
            req.turn_environment.environment.as_ref(),
        )
        .await;
        if let Some(sidecar) = metrics_sidecar.as_ref() {
            sidecar.install_output_env(&mut env);
        }
        #[cfg(unix)]
        let (env, runtime_path_prepends) = {
            let mut env = env;
            let mut runtime_path_prepends = RuntimePathPrepends::default();
            crate::tools::runtimes::apply_package_path_prepend(
                &mut env,
                &mut runtime_path_prepends,
            );
            if self.backend == ShellRuntimeBackend::ShellCommandZshFork
                && let Some(shell_zsh_path) = ctx.session.services.shell_zsh_path.as_deref()
            {
                apply_zsh_fork_path_prepend(&mut env, &mut runtime_path_prepends, shell_zsh_path);
            }
            (env, runtime_path_prepends)
        };
        #[cfg(not(unix))]
        let runtime_path_prepends = RuntimePathPrepends::default();
        let command = maybe_wrap_shell_lc_with_snapshot(
            &req.command,
            shell,
            shell_snapshot_location.as_ref(),
            &explicit_env_overrides,
            &env,
            &runtime_path_prepends,
        );
        let command = disable_powershell_profile_for_elevated_windows_sandbox(
            &command,
            req.shell_type.as_ref(),
            attempt.sandbox_requested,
            attempt.windows_sandbox_level,
        );
        let command = if matches!(shell.shell_type, ShellType::PowerShell) {
            prefix_powershell_script_with_utf8(&command)
        } else {
            command
        };

        let zsh_fork_output = if self.backend == ShellRuntimeBackend::ShellCommandZshFork {
            match zsh_fork_backend::maybe_run_shell_command(
                req,
                attempt,
                ctx,
                &command,
                metrics_sidecar.as_ref(),
            )
            .await?
            {
                Some(out) => Some(out),
                None => {
                    tracing::warn!(
                        "ZshFork backend specified, but conditions for using it were not met, falling back to normal execution",
                    );
                    None
                }
            }
        } else {
            None
        };
        let out = if let Some(out) = zsh_fork_output {
            out
        } else {
            let sidecar_permissions = metrics_sidecar
                .as_ref()
                .map(PluginMetricsSidecar::additional_permissions);
            let additional_permissions = merge_permission_profiles(
                req.additional_permissions.as_ref(),
                sidecar_permissions.as_ref(),
            );
            let command = build_sandbox_command(&command, &req.cwd, &env, additional_permissions)?;
            let mut expiration: crate::exec::ExecExpiration = req.timeout_ms.into();
            expiration = expiration.with_cancellation(req.cancellation_token.clone());
            if let Some(cancellation) = attempt.network_denial_cancellation_token.clone() {
                expiration = expiration.with_cancellation(cancellation);
            }
            let options = ExecOptions {
                expiration,
                capture_policy: ExecCapturePolicy::ShellTool,
            };
            let env = attempt
                .env_for(
                    command,
                    options,
                    managed_network,
                    Some(&req.turn_environment.selection.environment_id),
                )
                .map_err(ToolError::Codex)?;
            execute_env(env, Self::stdout_stream(ctx))
                .await
                .map_err(ToolError::Codex)?
        };
        finish_and_track_measurements(
            metrics_sidecar,
            out.exit_code,
            &ctx.session,
            &ctx.step_context.turn,
            &ctx.call_id,
        )
        .await;
        Ok(out)
    }
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
