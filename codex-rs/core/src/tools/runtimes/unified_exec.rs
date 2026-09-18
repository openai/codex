/*
Runtime: unified exec

Handles approval + sandbox orchestration for unified exec requests, delegating to
the process manager to spawn PTYs once an ExecRequest is prepared.
*/
use crate::exec::ExecCapturePolicy;
use crate::exec::ExecExpiration;
use crate::guardian::GUARDIAN_REVIEW_TIMEOUT;
use crate::guardian::GuardianNetworkAccessTrigger;
use crate::guardian::routes_approval_policy_to_guardian;
use crate::plugins::metrics::sidecar_for_command;
use crate::sandboxing::ExecOptions;
use crate::sandboxing::ExecServerEnvConfig;
use crate::sandboxing::SandboxPermissions;
use crate::session::turn_context::TurnEnvironment;
use crate::shell::ShellType;
use crate::shell_snapshot::ShellSnapshotSandbox;
use crate::shell_snapshot::snapshot_read_permissions;
use crate::tools::flat_tool_name;
use crate::tools::network_approval::NetworkApprovalSpec;
use crate::tools::runtimes::RuntimePathPrepends;
#[cfg(unix)]
use crate::tools::runtimes::apply_zsh_fork_path_prepend;
use crate::tools::runtimes::exec_env_for_sandbox_permissions;
use crate::tools::runtimes::maybe_wrap_shell_lc_with_snapshot;
use crate::tools::runtimes::prepare_powershell_command_for_elevated_windows_sandbox;
use crate::tools::runtimes::zsh_fork;
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
use crate::unified_exec::NoopSpawnLifecycle;
use crate::unified_exec::TerminalPermissions;
use crate::unified_exec::TerminalSandboxSource;
use crate::unified_exec::UnifiedExecError;
use crate::unified_exec::UnifiedExecProcess;
use crate::unified_exec::UnifiedExecProcessManager;
use codex_core_plugins::PluginMetricsSidecar;
use codex_network_proxy::CREDENTIAL_BROKER_ACTIVE_ENV_KEY;
use codex_network_proxy::ManagedNetworkSandboxContext;
use codex_network_proxy::NetworkProxy;
use codex_protocol::error::CodexErr;
use codex_protocol::error::SandboxErr;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxablePreference;
use codex_sandboxing::policy_transforms::merge_permission_profiles;
use codex_shell_command::powershell::prefix_powershell_script_with_utf8;
use codex_tools::UnifiedExecShellMode;
use codex_utils_path_uri::PathUri;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

// Allow 5s for Guardian cleanup and 5s for controller processing after review.
const REMOTE_NETWORK_POLICY_DECISION_MARGIN: Duration = Duration::from_secs(10);

/// Request payload used by the unified-exec runtime after approvals and
/// sandbox preferences have been resolved for the current turn.
#[derive(Clone, Debug)]
pub struct UnifiedExecRequest {
    pub command: Vec<String>,
    pub shell_type: ShellType,
    pub hook_command: String,
    pub process_id: i32,
    pub cwd: PathUri,
    pub sandbox_cwd: PathUri,
    pub turn_environment: TurnEnvironment,
    pub env: HashMap<String, String>,
    pub exec_server_env_config: Option<ExecServerEnvConfig>,
    pub shell_snapshot: Option<codex_exec_server::ShellSnapshotRequest>,
    pub explicit_env_overrides: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub tty: bool,
    pub sandbox_permissions: SandboxPermissions,
    pub additional_permissions: Option<AdditionalPermissionProfile>,
    #[cfg(unix)]
    pub additional_permissions_preapproved: bool,
    pub justification: Option<String>,
    pub exec_approval_requirement: ExecApprovalRequirement,
}

/// Cache key for approval decisions that can be reused across equivalent
/// unified-exec launches.
#[derive(serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct UnifiedExecApprovalKey {
    pub environment_id: String,
    pub executable: Option<String>,
    pub command: Vec<String>,
    pub cwd: PathUri,
    pub tty: bool,
    pub sandbox_permissions: SandboxPermissions,
    pub additional_permissions: Option<AdditionalPermissionProfile>,
}

/// Runtime adapter that keeps policy and sandbox orchestration on the
/// unified-exec side while delegating process startup to the manager.
pub struct UnifiedExecRuntime<'a> {
    manager: &'a UnifiedExecProcessManager,
    shell_mode: UnifiedExecShellMode,
}

pub(crate) struct UnifiedExecAttempt {
    pub(crate) process: UnifiedExecProcess,
    pub(crate) metrics_sidecar: Option<PluginMetricsSidecar>,
    pub(crate) permissions: TerminalPermissions,
}

fn unified_exec_options(
    network_denial_cancellation_token: Option<CancellationToken>,
) -> ExecOptions {
    let mut expiration = ExecExpiration::DefaultTimeout;
    if let Some(cancellation) = network_denial_cancellation_token {
        expiration = expiration.with_cancellation(cancellation);
    }
    ExecOptions {
        expiration,
        capture_policy: ExecCapturePolicy::ShellTool,
    }
}

fn build_unified_exec_sandbox_command(
    command: &[String],
    cwd: &PathUri,
    env: &HashMap<String, String>,
    managed_network: Option<ManagedNetworkSandboxContext>,
    additional_permissions: Option<AdditionalPermissionProfile>,
) -> Result<SandboxCommand, ToolError> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| ToolError::Rejected("command args are empty".to_string()))?;
    Ok(SandboxCommand {
        program: program.clone().into(),
        args: args.to_vec(),
        cwd: cwd.clone(),
        env: env.clone(),
        managed_network,
        additional_permissions,
    })
}

impl<'a> UnifiedExecRuntime<'a> {
    /// Creates a runtime bound to the shared unified-exec process manager.
    pub fn new(manager: &'a UnifiedExecProcessManager, shell_mode: UnifiedExecShellMode) -> Self {
        Self {
            manager,
            shell_mode,
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
    fn approval_action(
        &self,
        req: &UnifiedExecRequest,
        call_id: &str,
    ) -> std::io::Result<ApprovalAction> {
        Ok(ApprovalAction::ExecCommand {
            id: call_id.to_string(),
            environment_id: req.turn_environment.selection.environment_id.clone(),
            command: req.command.clone(),
            hook_command: req.hook_command.clone(),
            cwd: req.cwd.clone(),
            sandbox_permissions: req.sandbox_permissions,
            additional_permissions: req.additional_permissions.clone(),
            justification: req.justification.clone(),
            tty: req.tty,
            proposed_execpolicy_amendment: req
                .exec_approval_requirement
                .proposed_execpolicy_amendment()
                .cloned(),
        })
    }

    fn exec_approval_requirement(
        &self,
        req: &UnifiedExecRequest,
    ) -> Option<ExecApprovalRequirement> {
        Some(req.exec_approval_requirement.clone())
    }

    fn sandbox_permissions(&self, req: &UnifiedExecRequest) -> SandboxPermissions {
        req.sandbox_permissions
    }
}

impl<'a> ToolRuntime<UnifiedExecRequest, UnifiedExecAttempt> for UnifiedExecRuntime<'a> {
    fn turn_environment<'b>(&self, req: &'b UnifiedExecRequest) -> &'b TurnEnvironment {
        &req.turn_environment
    }

    fn uses_executor_managed_process_sandbox(&self, req: &UnifiedExecRequest) -> bool {
        req.turn_environment.environment.is_remote() || req.shell_snapshot.is_some()
    }

    fn sandbox_cwd<'b>(&self, req: &'b UnifiedExecRequest) -> Option<&'b PathUri> {
        Some(&req.sandbox_cwd)
    }

    fn network_approval_spec(
        &self,
        req: &UnifiedExecRequest,
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
        // Explicit full escalation bypasses controller and attachment-owned network proxies.
        // Denied-read restrictions above can still require a sandboxed launch.
        if sandbox_permissions.requires_escalated_permissions() {
            return None;
        }
        let network = req.network.clone();
        // No-proxy fast path; owners still need a spec for execution-only proxies.
        if network.is_none() && req.turn_environment.config().network_policy.is_none() {
            return None;
        }
        Some(NetworkApprovalSpec {
            network,
            tool_name: ctx.tool_name.clone(),
            trigger: GuardianNetworkAccessTrigger {
                call_id: ctx.call_id.clone(),
                tool_name: flat_tool_name(&ctx.tool_name).into_owned(),
                command: req.command.clone(),
                cwd: req.cwd.clone(),
                sandbox_permissions: req.sandbox_permissions,
                additional_permissions: req.additional_permissions.clone(),
                justification: req.justification.clone(),
                tty: Some(req.tty),
            },
            command: req.hook_command.clone(),
            environment_id: req.turn_environment.selection.environment_id.clone(),
            permission_profile: req.turn_environment.permission_profile().clone(),
            network_policy: req.turn_environment.config().network_policy.clone(),
        })
    }

    async fn run(
        &mut self,
        req: &UnifiedExecRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
    ) -> Result<UnifiedExecAttempt, ToolError> {
        let base_command = &req.command;
        let windows_sandbox_proxy_settings_mode = ctx.session.windows_sandbox_proxy_settings_mode;
        let session_shell = ctx.session.user_shell();
        let environment_shell = req
            .turn_environment
            .shell
            .as_ref()
            .unwrap_or(session_shell.as_ref());
        let (file_system_sandbox_policy, _) = attempt.permissions.to_runtime_permissions();
        let launch_sandbox_permissions = sandbox_permissions_preserving_denied_reads(
            req.sandbox_permissions,
            &file_system_sandbox_policy,
        );
        let managed_network = attempt.network_proxy(managed_network_for_sandbox_permissions(
            req.network.as_ref(),
            launch_sandbox_permissions,
        ));
        let environment_is_remote = req.turn_environment.environment.is_remote();
        let credential_broker_available = !environment_is_remote
            && req.network.is_some()
            && ctx
                .step_context
                .turn
                .config
                .permissions
                .network
                .as_ref()
                .is_some_and(crate::config::NetworkProxySpec::credential_broker_enabled);
        if credential_broker_available
            && managed_network.is_some()
            && matches!(self.shell_mode, UnifiedExecShellMode::ZshFork(_))
        {
            return Err(ToolError::Rejected(
                "credential brokerage does not yet support shell_zsh_fork; disable shell_zsh_fork to use the broker".to_string(),
            ));
        }
        let requested_shell = (credential_broker_available
            && (req.shell_type != environment_shell.shell_type
                || base_command.first().is_some_and(|path| {
                    path != environment_shell.shell_path.to_string_lossy().as_ref()
                })))
        .then(|| {
            base_command.first().map(|path| crate::shell::Shell {
                shell_type: req.shell_type,
                shell_path: PathBuf::from(path),
            })
        })
        .flatten();
        let shell = requested_shell.as_ref().unwrap_or(environment_shell);
        let shell_snapshot = if environment_is_remote
            || credential_broker_available
                && launch_sandbox_permissions.requires_escalated_permissions()
        {
            None
        } else {
            // TODO(anp): Make shell snapshot lookup accept PathUri.
            let native_cwd = req
                .cwd
                .to_abs_path()
                .map_err(|err| ToolError::Rejected(err.to_string()))?;
            req.turn_environment
                .shell_snapshot(
                    &native_cwd,
                    base_command,
                    shell,
                    &ctx.step_context.turn.config,
                    Some(ShellSnapshotSandbox::new(
                        attempt,
                        managed_network,
                        &req.turn_environment.selection.environment_id,
                        req.additional_permissions.as_ref(),
                    )),
                )
                .await
        };
        let shell_snapshot_location = shell_snapshot.as_ref().map(|snapshot| snapshot.path());
        let mut env = exec_env_for_sandbox_permissions(&req.env, launch_sandbox_permissions);
        let snapshot_credential_context = if let Some(snapshot) = shell_snapshot.as_ref()
            && (managed_network.is_some() || base_command.get(1).is_some_and(|flag| flag == "-lc"))
        {
            Some(
                snapshot
                    .restore_credentials(&mut env, req.turn_environment.shell_environment_policy()),
            )
        } else {
            None
        };
        let (mut env, managed_network_context, network_proxy_launch) = match managed_network {
            Some(network) if environment_is_remote => {
                let mut launch = network.remote_launch_config().await.map_err(|err| {
                    ToolError::Codex(CodexErr::Io(io::Error::other(err.to_string())))
                })?;
                if routes_approval_policy_to_guardian(
                    ctx.step_context.settings.approval_policy(),
                    ctx.step_context.settings.approvals_reviewer(),
                ) && network.remote_policy_decider().is_some()
                {
                    let timeout = ctx
                        .session
                        .hooks()
                        .max_permission_request_timeout()
                        .saturating_add(GUARDIAN_REVIEW_TIMEOUT)
                        .saturating_add(REMOTE_NETWORK_POLICY_DECISION_MARGIN);
                    launch.policy_decision_timeout_ms =
                        Some(u64::try_from(timeout.as_millis()).map_err(|_| {
                            ToolError::Rejected(
                                "remote network policy decision timeout exceeds protocol limit"
                                    .to_string(),
                            )
                        })?);
                }
                if !launch.proxy.enabled {
                    (env, None, None)
                } else {
                    let environment_info =
                        req.turn_environment
                            .environment
                            .info()
                            .await
                            .map_err(|err| {
                                ToolError::Codex(CodexErr::Io(io::Error::other(format!(
                                    "failed to query exec-server capabilities: {err}"
                                ))))
                            })?;
                    if !environment_info.capabilities.network_proxy_launch {
                        return Err(ToolError::Rejected(
                            "selected exec-server does not support executor-local network proxy launches"
                                .to_string(),
                        ));
                    }
                    (env, None, Some(launch))
                }
            }
            Some(network) => {
                let prepared = snapshot_credential_context
                    .unwrap_or_default()
                    .prepare_child_environment(
                        network,
                        env,
                        Some(&req.turn_environment.selection.environment_id),
                    )
                    .map_err(|err| {
                        ToolError::Codex(CodexErr::Io(io::Error::other(format!(
                            "failed to prepare network proxy for environment `{}`: {err}",
                            req.turn_environment.selection.environment_id
                        ))))
                    })?;
                (prepared.env, Some(prepared.sandbox_context), None)
            }
            None => (env, None, None),
        };
        if let Some(snapshot) = shell_snapshot.as_ref() {
            snapshot.restore_fail_open_aliases(
                &mut env,
                Some(&req.turn_environment.selection.environment_id),
            );
        }
        if managed_network.is_some()
            && !environment_is_remote
            && shell.shell_type == ShellType::Sh
            && env
                .get(CREDENTIAL_BROKER_ACTIVE_ENV_KEY)
                .is_some_and(|active| active == "1")
            && let Some(startup_env) = shell_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.protected_startup_env())
        {
            env.insert(
                super::SNAPSHOT_ORIGINAL_POSIX_ENV_ENV_KEY.to_string(),
                startup_env.to_string(),
            );
        }
        super::prepare_brokered_shell_snapshot_env(
            &mut env,
            shell_snapshot_location.as_ref(),
            shell,
        );
        if env
            .get(CREDENTIAL_BROKER_ACTIVE_ENV_KEY)
            .is_some_and(|active| active == "1")
            && let Some(snapshot) = shell_snapshot.as_ref()
        {
            for key in snapshot.startup_credential_keys() {
                if !super::is_valid_shell_variable_name(key) {
                    continue;
                }
                let (prefix, value) = match env.get(key) {
                    Some(value) => (super::SNAPSHOT_BROKERED_VALUE_ENV_PREFIX, value.clone()),
                    None => (super::SNAPSHOT_BROKERED_UNSET_ENV_PREFIX, "1".to_string()),
                };
                env.insert(format!("{prefix}{key}"), value);
            }
        }
        let mut explicit_env_overrides = req.explicit_env_overrides.clone();
        if let Some(network) = managed_network
            && !environment_is_remote
        {
            let broker = network.credential_broker_environment(&env);
            for key in broker
                .credential_keys
                .into_iter()
                .chain(broker.context_keys)
            {
                if !codex_network_proxy::is_credential_broker_provider_env_key(&key)
                    && let Some(value) = env.get(&key)
                {
                    explicit_env_overrides.insert(key, value.clone());
                }
            }
        }
        let metrics_sidecar = sidecar_for_command(
            ctx,
            &req.command,
            &req.cwd,
            req.turn_environment.environment.as_ref(),
        )
        .await;
        if let Some(sidecar) = metrics_sidecar.as_ref() {
            sidecar.install_output_env(&mut env);
        }
        #[cfg(unix)]
        let runtime_path_prepends = {
            let mut runtime_path_prepends = RuntimePathPrepends::default();
            if !environment_is_remote {
                crate::tools::runtimes::apply_package_path_prepend(
                    &mut env,
                    &mut runtime_path_prepends,
                );
            }
            if let UnifiedExecShellMode::ZshFork(zsh_fork_config) = &self.shell_mode {
                apply_zsh_fork_path_prepend(
                    &mut env,
                    &mut runtime_path_prepends,
                    zsh_fork_config.shell_zsh_path.as_path(),
                );
            }
            runtime_path_prepends
        };
        #[cfg(not(unix))]
        let runtime_path_prepends = RuntimePathPrepends::default();
        let mut command = if environment_is_remote {
            base_command.to_vec()
        } else {
            maybe_wrap_shell_lc_with_snapshot(
                base_command,
                shell,
                shell_snapshot_location.as_ref(),
                &explicit_env_overrides,
                &env,
                &runtime_path_prepends,
            )
        };
        let brokered_shell_snapshot_missing = !environment_is_remote
            && managed_network.is_some()
            && env
                .get(codex_network_proxy::CREDENTIAL_BROKER_ACTIVE_ENV_KEY)
                .is_some_and(|active| active == "1")
            && command
                .get(1)
                .is_some_and(|flag| matches!(flag.as_str(), "-lc" | "-c"))
            && !shell_snapshot_location
                .as_ref()
                .is_some_and(|snapshot| snapshot.exists());
        if brokered_shell_snapshot_missing && let Some(network) = managed_network {
            if cfg!(unix)
                && matches!(
                    shell.shell_type,
                    ShellType::Bash | ShellType::Zsh | ShellType::Sh
                )
            {
                return Err(ToolError::Rejected(
                    "credential brokerage could not create a protected shell snapshot".to_string(),
                ));
            }
            network.restore_and_disable_brokered_credentials(&mut env, &mut command);
        }
        if req.shell_snapshot.is_some() {
            let exports =
                runtime_path_prepends.shell_exports_after_snapshot(&explicit_env_overrides);
            if !exports.is_empty()
                && let Some(script) = command.get_mut(2)
            {
                *script = format!("{exports}\n{script}");
            }
        }
        if !environment_is_remote
            && matches!(shell.shell_type, ShellType::PowerShell | ShellType::Cmd)
            && env
                .get(CREDENTIAL_BROKER_ACTIVE_ENV_KEY)
                .is_some_and(|active| active == "1")
            && let Some(network) = managed_network
        {
            network.restore_and_disable_brokered_credentials(&mut env, &mut command);
        }
        let command = prepare_powershell_command_for_elevated_windows_sandbox(
            &command,
            Some(&req.shell_type),
            attempt.sandbox_requested,
            attempt.windows_sandbox_level,
            environment_is_remote,
        );
        let command = if matches!(req.shell_type, ShellType::PowerShell) {
            prefix_powershell_script_with_utf8(&command)
        } else {
            command
        };
        let sidecar_permissions = metrics_sidecar
            .as_ref()
            .map(PluginMetricsSidecar::additional_permissions);
        let snapshot_permissions = shell_snapshot_location
            .as_ref()
            .filter(|_| {
                env.get(codex_network_proxy::CREDENTIAL_BROKER_ACTIVE_ENV_KEY)
                    .is_some_and(|active| active == "1")
            })
            .and_then(|path| {
                snapshot_read_permissions(
                    path,
                    &req.turn_environment
                        .permission_profile_with_workspace_roots(),
                    attempt.sandbox_cwd,
                )
            });
        let internal_permissions =
            merge_permission_profiles(sidecar_permissions.as_ref(), snapshot_permissions.as_ref());
        let additional_permissions = merge_permission_profiles(
            req.additional_permissions.as_ref(),
            internal_permissions.as_ref(),
        );
        let permissions = TerminalPermissions::for_launch(
            &req.turn_environment,
            &ctx.step_context.turn,
            if self.uses_executor_managed_process_sandbox(req) {
                TerminalSandboxSource::Executor
            } else {
                TerminalSandboxSource::Native
            },
            if attempt.is_escalated() {
                SandboxPermissions::RequireEscalated
            } else {
                SandboxPermissions::UseDefault
            },
            req.additional_permissions.as_ref(),
            internal_permissions.as_ref(),
        );

        if let UnifiedExecShellMode::ZshFork(zsh_fork_config) = &self.shell_mode {
            let command = build_unified_exec_sandbox_command(
                &command,
                &req.cwd,
                &env,
                managed_network_context.clone(),
                additional_permissions.clone(),
            )
            .map_err(|error| match error {
                ToolError::Rejected(_) => {
                    ToolError::Rejected("missing command line for PTY".to_string())
                }
                error @ ToolError::Codex(_) => error,
            })?;
            let options = unified_exec_options(attempt.network_denial_cancellation_token.clone());
            let mut exec_env = attempt
                .env_for(
                    command,
                    options,
                    managed_network,
                    Some(&req.turn_environment.selection.environment_id),
                )
                .map_err(ToolError::Codex)?;
            exec_env.exec_server_env_config = req.exec_server_env_config.clone();
            match zsh_fork::maybe_prepare_unified_exec(req, attempt, ctx, exec_env, zsh_fork_config)
                .await?
            {
                Some(prepared) => {
                    if req.turn_environment.environment.is_remote() {
                        return Err(ToolError::Rejected(
                            "unified_exec zsh-fork is not supported for remote environments"
                                .to_string(),
                        ));
                    }
                    let mut process = self
                        .manager
                        .open_session_with_prepared_exec_env(
                            req.process_id,
                            &prepared.exec_request,
                            Some(ctx),
                            windows_sandbox_proxy_settings_mode,
                            /*network_policy_decider*/ None,
                            req.tty,
                            prepared.spawn_lifecycle,
                            req.turn_environment.environment.as_ref(),
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
                        })?;
                    process._shell_snapshot = shell_snapshot;
                    return Ok(UnifiedExecAttempt {
                        process,
                        metrics_sidecar,
                        permissions,
                    });
                }
                None => {
                    tracing::warn!(
                        "UnifiedExec ZshFork backend specified, but conditions for using it were not met, falling back to direct execution",
                    );
                }
            }
        }
        let command = build_unified_exec_sandbox_command(
            &command,
            &req.cwd,
            &env,
            managed_network_context,
            additional_permissions,
        )
        .map_err(|error| match error {
            ToolError::Rejected(_) => {
                ToolError::Rejected("missing command line for PTY".to_string())
            }
            error @ ToolError::Codex(_) => error,
        })?;
        let options = unified_exec_options(attempt.network_denial_cancellation_token.clone());
        let mut process = self
            .manager
            .open_session_with_exec_env(
                req.process_id,
                ctx,
                command,
                options,
                attempt,
                managed_network,
                network_proxy_launch,
                /*environment_id*/ Some(&req.turn_environment.selection.environment_id),
                req.exec_server_env_config.clone(),
                req.shell_snapshot.clone(),
                windows_sandbox_proxy_settings_mode,
                req.tty,
                Box::new(NoopSpawnLifecycle),
                req.turn_environment.environment.as_ref(),
            )
            .await?;
        process._shell_snapshot = shell_snapshot;
        Ok(UnifiedExecAttempt {
            process,
            metrics_sidecar,
            permissions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PermissionProfileSnapshot;
    use crate::environment_selection::EnvironmentConfigOrigin;
    use crate::exec::DEFAULT_EXEC_COMMAND_TIMEOUT_MS;
    use crate::tools::sandboxing::ToolRuntime;
    use codex_exec_server::Environment;
    use codex_exec_server::LOCAL_ENVIRONMENT_ID;
    use codex_protocol::config_types::WindowsSandboxLevel;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::protocol::EnvironmentConfig;
    use codex_protocol::protocol::EnvironmentConfigState;
    use codex_protocol::protocol::TurnEnvironmentSelection;
    use codex_tools::ZshForkConfig;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_utils_path_uri::PathUri;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::tempdir;

    fn test_turn_environment(cwd: PathUri) -> TurnEnvironment {
        TurnEnvironment::new(
            TurnEnvironmentSelection {
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd,
                workspace_roots: Vec::new(),
                config: EnvironmentConfigState::Ready(EnvironmentConfig {
                    allow_login_shell: true,
                    workspace_roots: Vec::new(),
                    windows_sandbox_level: WindowsSandboxLevel::Disabled,
                    windows_sandbox_type: codex_sandboxing::SandboxType::None,
                    windows_sandbox_private_desktop: true,
                    use_legacy_landlock: false,
                    permission_profile: PermissionProfileSnapshot::legacy(
                        PermissionProfile::read_only(),
                    ),
                    shell_environment_policy: Default::default(),
                    exec_policy: None,
                    mcp_policy: None,
                    network_policy: None,
                    selected_capability_roots: Vec::new(),
                }),
            },
            EnvironmentConfigOrigin::Thread,
            Arc::new(Environment::default_for_tests()),
            /*shell*/ None,
        )
    }

    #[test]
    fn unified_exec_options_combines_default_timeout_with_network_denial_cancellation() {
        let cancellation = CancellationToken::new();
        let options = unified_exec_options(Some(cancellation.clone()));

        assert_eq!(options.capture_policy, ExecCapturePolicy::ShellTool);
        match options.expiration {
            ExecExpiration::TimeoutOrCancellation {
                timeout,
                cancellation: actual,
            } => {
                assert_eq!(
                    timeout,
                    Duration::from_millis(DEFAULT_EXEC_COMMAND_TIMEOUT_MS)
                );
                cancellation.cancel();
                assert!(actual.is_cancelled());
            }
            other => panic!("expected timeout-or-cancellation expiration, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn approval_key_includes_environment_id() {
        let manager = UnifiedExecProcessManager::default();
        let runtime = UnifiedExecRuntime::new(&manager, UnifiedExecShellMode::Direct);
        let mut request = test_request(
            SandboxPermissions::UseDefault,
            ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
        );
        request.turn_environment.selection.environment_id = "remote".to_string();
        let original_key = runtime
            .approval_action(&request, "call-1")
            .expect("build approval action")
            .cache_keys();
        request.turn_environment.selection.environment_id = "other".to_string();
        let other_key = runtime
            .approval_action(&request, "call-1")
            .expect("build approval action")
            .cache_keys();

        assert_ne!(original_key, other_key);
    }

    #[tokio::test]
    async fn unified_exec_uses_the_trusted_sandbox_cwd() {
        let cwd_dir = tempdir().expect("create process temp dir");
        let sandbox_dir = tempdir().expect("create sandbox temp dir");
        let cwd =
            AbsolutePathBuf::try_from(cwd_dir.path().to_path_buf()).expect("absolute temp dir");
        let sandbox_cwd = AbsolutePathBuf::try_from(sandbox_dir.path().to_path_buf())
            .expect("absolute sandbox temp dir");
        let manager = UnifiedExecProcessManager::default();
        let runtime = UnifiedExecRuntime::new(&manager, UnifiedExecShellMode::Direct);
        let request = UnifiedExecRequest {
            command: vec!["pwd".to_string()],
            shell_type: ShellType::Sh,
            hook_command: "pwd".to_string(),
            process_id: 1000,
            cwd: cwd.into(),
            sandbox_cwd: sandbox_cwd.clone().into(),
            turn_environment: test_turn_environment(sandbox_cwd.clone().into()),
            env: HashMap::new(),
            exec_server_env_config: None,
            shell_snapshot: None,
            explicit_env_overrides: HashMap::new(),
            network: None,
            tty: false,
            sandbox_permissions: SandboxPermissions::UseDefault,
            additional_permissions: None,
            #[cfg(unix)]
            additional_permissions_preapproved: false,
            justification: None,
            exec_approval_requirement: ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
        };

        assert_eq!(
            runtime.sandbox_cwd(&request),
            Some(&PathUri::from_abs_path(&sandbox_cwd))
        );
    }

    #[tokio::test]
    async fn zsh_fork_first_attempt_preserves_parent_sandbox_override() {
        let manager = UnifiedExecProcessManager::default();
        let request = test_request(
            SandboxPermissions::RequireEscalated,
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                proposed_execpolicy_amendment: None,
            },
        );
        let direct_runtime = UnifiedExecRuntime::new(&manager, UnifiedExecShellMode::Direct);
        let zsh_fork_runtime = UnifiedExecRuntime::new(&manager, zsh_fork_mode());

        assert_eq!(
            direct_runtime.sandbox_permissions(&request),
            SandboxPermissions::RequireEscalated,
            "direct unified exec should preserve a parent require_escalated request"
        );
        assert_eq!(
            zsh_fork_runtime.sandbox_permissions(&request),
            SandboxPermissions::RequireEscalated,
            "zsh-fork unified exec should preserve the same parent require_escalated request"
        );
    }

    #[tokio::test]
    async fn zsh_fork_first_attempt_preserves_additional_permissions_request() {
        let manager = UnifiedExecProcessManager::default();
        let request = test_request(
            SandboxPermissions::WithAdditionalPermissions,
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                proposed_execpolicy_amendment: None,
            },
        );
        let zsh_fork_runtime = UnifiedExecRuntime::new(&manager, zsh_fork_mode());

        assert_eq!(
            zsh_fork_runtime.sandbox_permissions(&request),
            SandboxPermissions::WithAdditionalPermissions,
            "zsh-fork unified exec should keep bounded additional-permissions requests sandboxed"
        );
    }

    #[tokio::test]
    async fn zsh_fork_execpolicy_allow_preserves_parent_sandbox_override() {
        let manager = UnifiedExecProcessManager::default();
        let request = test_request(
            SandboxPermissions::UseDefault,
            ExecApprovalRequirement::Skip {
                bypass_sandbox: true,
                proposed_execpolicy_amendment: None,
            },
        );
        let runtime = UnifiedExecRuntime::new(&manager, zsh_fork_mode());

        assert_eq!(
            runtime.exec_approval_requirement(&request),
            Some(ExecApprovalRequirement::Skip {
                bypass_sandbox: true,
                proposed_execpolicy_amendment: None,
            }),
            "zsh-fork unified exec should preserve exec-policy allow decisions that bypass the sandbox"
        );
    }

    fn test_request(
        sandbox_permissions: SandboxPermissions,
        exec_approval_requirement: ExecApprovalRequirement,
    ) -> UnifiedExecRequest {
        let cwd = AbsolutePathBuf::try_from(std::env::current_dir().unwrap())
            .expect("current dir is absolute");
        UnifiedExecRequest {
            command: vec!["zsh".to_string(), "-c".to_string(), "echo hi".to_string()],
            shell_type: ShellType::Zsh,
            hook_command: "echo hi".to_string(),
            process_id: 1000,
            cwd: cwd.clone().into(),
            sandbox_cwd: cwd.clone().into(),
            turn_environment: test_turn_environment(cwd.into()),
            env: HashMap::new(),
            exec_server_env_config: None,
            shell_snapshot: None,
            explicit_env_overrides: HashMap::new(),
            network: None,
            tty: false,
            sandbox_permissions,
            additional_permissions: None,
            #[cfg(unix)]
            additional_permissions_preapproved: false,
            justification: None,
            exec_approval_requirement,
        }
    }

    fn zsh_fork_mode() -> UnifiedExecShellMode {
        let cwd = std::env::current_dir().expect("read current dir");
        UnifiedExecShellMode::ZshFork(ZshForkConfig {
            shell_zsh_path: AbsolutePathBuf::try_from(cwd.join("zsh")).expect("absolute zsh path"),
            main_execve_wrapper_exe: AbsolutePathBuf::try_from(cwd.join("execve-wrapper"))
                .expect("absolute wrapper path"),
        })
    }
}
