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
use crate::tools::handlers::unified_exec::preparation::PreparedExecCommand;
use crate::tools::network_approval::NetworkApprovalSpec;
use crate::tools::network_approval::remote_network_proxy_launch;
use crate::tools::runtimes::RuntimePathPrepends;
use crate::tools::runtimes::exec_env_for_sandbox_permissions;
use crate::tools::runtimes::maybe_wrap_shell_lc_with_snapshot;
use crate::tools::runtimes::prepare_powershell_command_for_windows_sandbox;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalAction;
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::executor_windows_sandbox_selection;
use crate::tools::sandboxing::managed_network_for_sandbox_permissions;
use crate::tools::sandboxing::sandbox_permissions_preserving_denied_reads;
use crate::unified_exec::TerminalPermissions;
use crate::unified_exec::TerminalSandboxSource;
use crate::unified_exec::UnifiedExecProcess;
use crate::unified_exec::UnifiedExecProcessManager;
use codex_core_plugins::PluginMetricsSidecar;
use codex_features::Feature;
use codex_network_proxy::CREDENTIAL_BROKER_ACTIVE_ENV_KEY;
use codex_network_proxy::ManagedNetworkSandboxContext;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::RemoteCredentialMasking;
use codex_network_proxy::RemoteNetworkProxyConfig;
use codex_protocol::error::CodexErr;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::WarningEvent;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxablePreference;
use codex_sandboxing::policy_transforms::merge_permission_profiles;
use codex_shell_command::powershell::prefix_powershell_script_with_utf8;
use codex_utils_path_uri::PathUri;
use std::collections::HashMap;
use std::io;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

mod launch;
mod snapshot_metrics;

use launch::with_launch_failure_events;

// Allow 5s for Guardian cleanup and 5s for controller processing after review.
const REMOTE_NETWORK_POLICY_DECISION_MARGIN: Duration = Duration::from_secs(10);

/// Request payload used by the unified-exec runtime after approvals and
/// sandbox preferences have been resolved for the current turn.
#[derive(Clone, Debug)]
pub struct UnifiedExecRequest<'a> {
    pub(crate) prepared: &'a PreparedExecCommand,
    pub process_id: i32,
    pub env: HashMap<String, String>,
    pub exec_server_env_config: Option<ExecServerEnvConfig>,
    pub shell_snapshot: Option<codex_exec_server::ShellSnapshotRequest>,
    pub explicit_env_overrides: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
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
    pub fn new(manager: &'a UnifiedExecProcessManager) -> Self {
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

impl Approvable<UnifiedExecRequest<'_>> for UnifiedExecRuntime<'_> {
    fn approval_action(
        &self,
        req: &UnifiedExecRequest<'_>,
        call_id: &str,
    ) -> std::io::Result<ApprovalAction> {
        Ok(req
            .prepared
            .approval_action(call_id, &req.exec_approval_requirement))
    }

    fn exec_approval_requirement(
        &self,
        req: &UnifiedExecRequest<'_>,
    ) -> Option<ExecApprovalRequirement> {
        Some(req.exec_approval_requirement.clone())
    }

    fn sandbox_permissions(&self, req: &UnifiedExecRequest<'_>) -> SandboxPermissions {
        req.prepared.permissions.sandbox_permissions
    }
}

impl<'a> ToolRuntime<UnifiedExecRequest<'_>, UnifiedExecAttempt> for UnifiedExecRuntime<'a> {
    fn turn_environment<'b>(&self, req: &'b UnifiedExecRequest<'_>) -> &'b TurnEnvironment {
        &req.prepared.turn_environment
    }

    fn uses_executor_managed_process_sandbox(&self, req: &UnifiedExecRequest<'_>) -> bool {
        req.prepared.turn_environment.environment.is_remote() || req.shell_snapshot.is_some()
    }

    fn sandbox_cwd<'b>(&self, req: &'b UnifiedExecRequest<'_>) -> Option<&'b PathUri> {
        Some(req.prepared.turn_environment.cwd())
    }

    fn network_approval_spec(
        &self,
        req: &UnifiedExecRequest<'_>,
        ctx: &ToolCtx,
    ) -> Option<NetworkApprovalSpec> {
        let prepared = req.prepared;
        let file_system_sandbox_policy = prepared
            .turn_environment
            .permission_profile()
            .file_system_sandbox_policy();
        let sandbox_permissions = sandbox_permissions_preserving_denied_reads(
            prepared.permissions.sandbox_permissions,
            &file_system_sandbox_policy,
        );
        // Explicit full escalation bypasses controller and attachment-owned network proxies.
        // Denied-read restrictions above can still require a sandboxed launch.
        if sandbox_permissions.requires_escalated_permissions() {
            return None;
        }
        let network = req.network.clone();
        // No-proxy fast path; owners still need a spec for execution-only proxies.
        if network.is_none() && prepared.turn_environment.config().network_policy.is_none() {
            return None;
        }
        Some(NetworkApprovalSpec {
            network,
            tool_name: ctx.tool_name.clone(),
            trigger: GuardianNetworkAccessTrigger {
                call_id: ctx.call_id.clone(),
                tool_name: flat_tool_name(&ctx.tool_name).into_owned(),
                command: prepared.resolved.command.clone(),
                cwd: prepared.cwd.clone(),
                sandbox_permissions: prepared.permissions.sandbox_permissions,
                additional_permissions: prepared.permissions.additional_permissions.clone(),
                justification: prepared.justification.clone(),
                tty: Some(prepared.tty),
            },
            command: prepared.hook_command.clone(),
            environment_id: prepared.turn_environment.selection.environment_id.clone(),
            permission_profile: prepared.turn_environment.permission_profile().clone(),
            network_policy: prepared.turn_environment.config().network_policy.clone(),
        })
    }

    async fn run(
        &mut self,
        req: &UnifiedExecRequest<'_>,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
    ) -> Result<UnifiedExecAttempt, ToolError> {
        let prepared = req.prepared;
        let base_command = &prepared.resolved.command;
        let requested_shell = &prepared.resolved.shell.shell;
        let windows_sandbox_proxy_settings_mode = ctx.session.windows_sandbox_proxy_settings_mode;
        let session_shell = ctx.session.user_shell();
        let environment_shell = prepared
            .turn_environment
            .shell
            .as_ref()
            .unwrap_or(session_shell.as_ref());
        let (file_system_sandbox_policy, _) = attempt.permissions.to_runtime_permissions();
        let launch_sandbox_permissions = sandbox_permissions_preserving_denied_reads(
            prepared.permissions.sandbox_permissions,
            &file_system_sandbox_policy,
        );
        let managed_network = attempt.network_proxy(managed_network_for_sandbox_permissions(
            req.network.as_ref(),
            launch_sandbox_permissions,
        ));
        let environment_is_remote = prepared.turn_environment.environment.is_remote();
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
        let shell = if credential_broker_available {
            requested_shell
        } else {
            environment_shell
        };
        let snapshot_metrics =
            snapshot_metrics::SnapshotMetrics::start(req, &ctx.step_context.turn.config);
        let shell_snapshot = if environment_is_remote
            || credential_broker_available
                && launch_sandbox_permissions.requires_escalated_permissions()
        {
            None
        } else {
            // TODO(anp): Make shell snapshot lookup accept PathUri.
            let native_cwd = prepared
                .cwd
                .to_abs_path()
                .map_err(|err| ToolError::Rejected(err.to_string()))?;
            prepared
                .turn_environment
                .shell_snapshot(
                    &native_cwd,
                    base_command,
                    shell,
                    &ctx.step_context.turn.config,
                    Some(ShellSnapshotSandbox::new(
                        attempt,
                        managed_network,
                        &prepared.turn_environment.selection.environment_id,
                        prepared.permissions.additional_permissions.as_ref(),
                    )),
                )
                .await
        };
        let snapshot_wait = snapshot_metrics
            .as_ref()
            .map(|metrics| metrics.started_at.elapsed());
        let shell_snapshot_location = shell_snapshot.as_ref().map(|snapshot| snapshot.path());
        let mut env = exec_env_for_sandbox_permissions(&req.env, launch_sandbox_permissions);
        let snapshot_credential_context = if let Some(snapshot) = shell_snapshot.as_ref()
            && (managed_network.is_some() || prepared.resolved.shell.is_posix_login())
        {
            Some(snapshot.restore_credentials(
                &mut env,
                prepared.turn_environment.shell_environment_policy(),
            ))
        } else {
            None
        };
        let (mut env, managed_network_context, network_proxy_launch) = match managed_network {
            Some(network) if environment_is_remote => {
                let (mut launch, credential_masking) = remote_network_proxy_launch(
                    &prepared.turn_environment,
                    network,
                    &ctx.step_context.turn,
                )
                .await?;
                warn_if_remote_credential_masking_unavailable(
                    ctx,
                    req,
                    credential_masking,
                    &launch.proxy,
                )
                .await;
                if routes_approval_policy_to_guardian(
                    ctx.step_context.settings.approval_policy(),
                    ctx.step_context.settings.approvals_reviewer(),
                ) && network
                    .remote_policy_decider(launch.proxy.allow_local_binding)
                    .is_some()
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
                (env, None, launch.proxy.enabled.then_some(launch))
            }
            Some(network) => {
                let child_environment = snapshot_credential_context
                    .unwrap_or_default()
                    .prepare_child_environment(
                        network,
                        env,
                        Some(&prepared.turn_environment.selection.environment_id),
                    )
                    .map_err(|err| {
                        ToolError::Codex(CodexErr::Io(io::Error::other(format!(
                            "failed to prepare network proxy for environment `{}`: {err}",
                            prepared.turn_environment.selection.environment_id
                        ))))
                    })?;
                (
                    child_environment.env,
                    Some(child_environment.sandbox_context),
                    None,
                )
            }
            None => (env, None, None),
        };
        if let Some(snapshot) = shell_snapshot.as_ref() {
            snapshot.restore_fail_open_aliases(
                &mut env,
                Some(&prepared.turn_environment.selection.environment_id),
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
            &prepared.resolved.command,
            &prepared.cwd,
            prepared.turn_environment.environment.as_ref(),
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
            runtime_path_prepends
        };
        #[cfg(not(unix))]
        let runtime_path_prepends = RuntimePathPrepends::default();
        // Restore the executor's PATH directories inside the shell so nested commands can
        // still find bundled tools even if login startup clears the inherited PATH.
        // `base_command` already carries the requested login mode; keep it if setup is unavailable.
        // `hook_command` is the raw script the helper needs to prepend setup inside that same shell.
        let mut command = base_command.to_vec();
        if ctx.session.enabled(Feature::LoginShellPackagePath)
            && prepared.resolved.shell.is_posix_login()
            && !explicit_env_overrides.contains_key("PATH")
            && let Ok(info) = prepared.turn_environment.environment.info().await
            && let Some(command_with_path_prepends) =
                prepared.resolved.shell.derive_exec_args_with_path_prepends(
                    &prepared.hook_command,
                    &info.prepend_path_dirs,
                )
        {
            command = command_with_path_prepends;
        }
        let mut snapshot_used = false;
        if !environment_is_remote {
            let wrapped = maybe_wrap_shell_lc_with_snapshot(
                &command,
                shell,
                shell_snapshot_location.as_ref(),
                &explicit_env_overrides,
                &env,
                &runtime_path_prepends,
            );
            snapshot_used = wrapped != command;
            command = wrapped;
        }
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
            network.restore_and_disable_brokered_credentials(&mut env, &mut command);
            let message = "Credential masking is unavailable because a protected shell snapshot could not be created. This command will run with unmasked credentials.";
            tracing::warn!("{message}");
            ctx.session
                .send_event(
                    ctx.step_context.turn.as_ref(),
                    EventMsg::Warning(WarningEvent {
                        message: message.to_string(),
                    }),
                )
                .await;
        }
        if let Some(metrics) = snapshot_metrics
            && let Some(wait) = snapshot_wait
        {
            let outcome = if snapshot_used { "used" } else { "fallback" };
            metrics.record(&ctx.step_context.session_telemetry, wait, outcome);
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
        // The executor selection is Disabled for non-Windows target paths, so this
        // preparation is a no-op for PowerShell on other platforms.
        let command = prepare_powershell_command_for_windows_sandbox(
            &command,
            Some(&requested_shell.shell_type),
            attempt.sandbox_requested,
            executor_windows_sandbox_selection(
                attempt.windows_sandbox_type,
                attempt.windows_sandbox_level,
                attempt.sandbox_cwd,
            ),
            environment_is_remote,
        );
        let command = if matches!(requested_shell.shell_type, ShellType::PowerShell) {
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
                    &prepared
                        .turn_environment
                        .permission_profile_with_workspace_roots(),
                    attempt.sandbox_cwd,
                )
            });
        let internal_permissions =
            merge_permission_profiles(sidecar_permissions.as_ref(), snapshot_permissions.as_ref());
        let additional_permissions = merge_permission_profiles(
            prepared.permissions.additional_permissions.as_ref(),
            internal_permissions.as_ref(),
        );
        let permissions = TerminalPermissions::for_launch(
            &prepared.turn_environment,
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
            prepared.permissions.additional_permissions.as_ref(),
            internal_permissions.as_ref(),
        );
        let baseline_file_system = prepared
            .turn_environment
            .permission_profile()
            .file_system_sandbox_policy();
        let permissions = if baseline_file_system.has_denied_read_restrictions()
            && attempt.exec_server_permissions.file_system_sandbox_policy() != baseline_file_system
        {
            permissions.with_filesystem_escalation()
        } else {
            permissions
        };

        let command = build_unified_exec_sandbox_command(
            &command,
            &prepared.cwd,
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
        let process = self
            .manager
            .open_session_with_exec_env(
                req.process_id,
                ctx,
                command,
                options,
                attempt,
                managed_network,
                network_proxy_launch,
                /*environment_id*/ Some(&prepared.turn_environment.selection.environment_id),
                req.exec_server_env_config.clone(),
                req.shell_snapshot.clone(),
                windows_sandbox_proxy_settings_mode,
                prepared.tty,
                prepared.turn_environment.environment.as_ref(),
            )
            .await;
        let mut process = with_launch_failure_events(process, prepared, ctx).await?;
        process._shell_snapshot = shell_snapshot;
        Ok(UnifiedExecAttempt {
            process,
            metrics_sidecar,
            permissions,
        })
    }
}

async fn warn_if_remote_credential_masking_unavailable(
    ctx: &ToolCtx,
    req: &UnifiedExecRequest<'_>,
    credential_masking: RemoteCredentialMasking,
    proxy_config: &RemoteNetworkProxyConfig,
) {
    if !ctx
        .step_context
        .turn
        .config
        .features
        .enabled(Feature::CredentialMasking)
    {
        return;
    }

    let prepared = req.prepared;
    let environment_id = &prepared.turn_environment.selection.environment_id;
    let message = if credential_masking == RemoteCredentialMasking::Disabled {
        format!(
            "Credential masking is unavailable on environment `{environment_id}` because its exec-server does not support it. This command will run with unmasked credentials."
        )
    } else if !proxy_config.credential_masking_enabled() {
        format!(
            "Credential masking is unavailable for this network proxy configuration on environment `{environment_id}`. This command will run with unmasked credentials."
        )
    } else if !matches!(
        prepared.resolved.shell.shell.shell_type,
        ShellType::Bash | ShellType::Zsh
    ) {
        format!(
            "Credential masking is unavailable for shell `{}` on environment `{environment_id}`. This command will run with unmasked credentials.",
            prepared.resolved.shell.shell.name(),
        )
    } else if match prepared.resolved.shell.shell.shell_type {
        ShellType::Bash => prepared.resolved.shell.use_login_shell || req.shell_snapshot.is_some(),
        ShellType::Zsh => !prepared.resolved.shell.use_login_shell || req.shell_snapshot.is_none(),
        ShellType::Sh | ShellType::PowerShell | ShellType::Cmd => true,
    } {
        format!(
            "Credential masking is unavailable for this shell startup on environment `{environment_id}`. This command will run with unmasked credentials."
        )
    } else {
        return;
    };

    ctx.session
        .send_event(
            ctx.step_context.turn.as_ref(),
            EventMsg::Warning(WarningEvent { message }),
        )
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PermissionProfileSnapshot;
    use crate::environment_selection::EnvironmentConfigOrigin;
    use crate::exec::DEFAULT_EXEC_COMMAND_TIMEOUT_MS;
    use crate::shell::Shell;
    use crate::shell::ShellInvocation;
    use crate::tools::handlers::EffectiveAdditionalPermissions;
    use crate::tools::handlers::unified_exec::ResolvedCommand;
    use crate::tools::sandboxing::ToolRuntime;
    use codex_exec_server::Environment;
    use codex_exec_server::LOCAL_ENVIRONMENT_ID;
    use codex_protocol::config_types::WindowsSandboxLevel;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::protocol::EnvironmentConfig;
    use codex_protocol::protocol::EnvironmentConfigState;
    use codex_protocol::protocol::TurnEnvironmentSelection;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_utils_path_uri::PathUri;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::tempdir;

    fn test_turn_environment(cwd: PathUri) -> TurnEnvironment {
        TurnEnvironment::new(
            TurnEnvironmentSelection {
                selected_capability_roots: Default::default(),
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd,
                workspace_roots: Vec::new(),
                config: EnvironmentConfigState::Ready(EnvironmentConfig {
                    environment_hooks: Default::default(),
                    allow_login_shell: true,
                    workspace_roots: Vec::new(),
                    windows_sandbox_level: WindowsSandboxLevel::Disabled,
                    windows_sandbox_type: codex_sandboxing::SandboxType::None,
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
        let runtime = UnifiedExecRuntime::new(&manager);
        let mut prepared = test_prepared_command();
        let approval_key = |prepared: &PreparedExecCommand| {
            runtime
                .approval_action(&test_request(prepared), "call-1")
                .expect("build approval action")
                .cache_keys()
        };
        prepared.turn_environment.selection.environment_id = "remote".to_string();
        let original_key = approval_key(&prepared);
        prepared.turn_environment.selection.environment_id = "other".to_string();
        let other_key = approval_key(&prepared);

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
        let runtime = UnifiedExecRuntime::new(&manager);
        let mut prepared = test_prepared_command();
        prepared.cwd = cwd.into();
        prepared.turn_environment = test_turn_environment(sandbox_cwd.clone().into());
        let request = test_request(&prepared);

        assert_eq!(
            runtime.sandbox_cwd(&request),
            Some(&PathUri::from_abs_path(&sandbox_cwd))
        );
    }

    fn test_prepared_command() -> PreparedExecCommand {
        let cwd = AbsolutePathBuf::try_from(std::env::current_dir().unwrap())
            .expect("current dir is absolute");
        PreparedExecCommand {
            resolved: ResolvedCommand {
                command: vec!["zsh".to_string(), "-c".to_string(), "echo hi".to_string()],
                shell: ShellInvocation {
                    shell: Shell {
                        shell_type: ShellType::Zsh,
                        shell_path: "zsh".into(),
                    },
                    use_login_shell: false,
                },
            },
            hook_command: "echo hi".to_string(),
            turn_environment: test_turn_environment(cwd.clone().into()),
            cwd: cwd.into(),
            permissions: EffectiveAdditionalPermissions {
                sandbox_permissions: SandboxPermissions::UseDefault,
                additional_permissions: None,
                permissions_preapproved: false,
            },
            justification: None,
            prefix_rule: None,
            tty: false,
            yield_time_ms: 10_000,
            max_output_tokens: None,
            completion_timeout: None,
        }
    }

    fn test_request(prepared: &PreparedExecCommand) -> UnifiedExecRequest<'_> {
        UnifiedExecRequest {
            prepared,
            process_id: 1000,
            env: HashMap::new(),
            exec_server_env_config: None,
            shell_snapshot: None,
            explicit_env_overrides: HashMap::new(),
            network: None,
            exec_approval_requirement: ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
        }
    }
}
