/*
Runtime: unified exec

Handles approval + sandbox orchestration for unified exec requests, delegating to
the process manager to spawn PTYs once an ExecRequest is prepared.
*/
use crate::command_canonicalization::canonicalize_command_for_approval;
use crate::exec::ExecCapturePolicy;
use crate::exec::ExecExpiration;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::GuardianNetworkAccessTrigger;
use crate::guardian::review_approval_request;
use crate::plugin_script_lifecycle::PluginScriptExecution;
use crate::sandboxing::ExecOptions;
use crate::sandboxing::ExecServerEnvConfig;
use crate::sandboxing::SandboxPermissions;
use crate::session::turn_context::TurnEnvironment;
use crate::shell::ShellType;
use crate::tools::flat_tool_name;
use crate::tools::network_approval::NetworkApprovalMode;
use crate::tools::network_approval::NetworkApprovalSpec;
use crate::tools::runtimes::RuntimePathPrepends;
#[cfg(unix)]
use crate::tools::runtimes::apply_zsh_fork_path_prepend;
use crate::tools::runtimes::disable_powershell_profile_for_elevated_windows_sandbox;
use crate::tools::runtimes::exec_env_for_sandbox_permissions;
use crate::tools::runtimes::maybe_wrap_shell_lc_with_snapshot;
use crate::tools::runtimes::shell::zsh_fork_backend;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ExecApprovalRequirement;
use crate::tools::sandboxing::PermissionRequestPayload;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use crate::tools::sandboxing::managed_network_for_sandbox_permissions;
use crate::tools::sandboxing::sandbox_permissions_preserving_denied_reads;
use crate::tools::sandboxing::with_cached_approval;
use crate::unified_exec::NoopSpawnLifecycle;
use crate::unified_exec::SpawnLifecycle;
use crate::unified_exec::SpawnLifecycleHandle;
use crate::unified_exec::UnifiedExecError;
use crate::unified_exec::UnifiedExecProcess;
use crate::unified_exec::UnifiedExecProcessManager;
use codex_network_proxy::NetworkProxy;
use codex_protocol::error::CodexErr;
use codex_protocol::error::SandboxErr;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::protocol::ReviewDecision;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxablePreference;
use codex_shell_command::powershell::prefix_powershell_script_with_utf8;
use codex_tools::UnifiedExecShellMode;
use codex_utils_path_uri::PathUri;
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::error;

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

/// Lifecycle callback surface that lets unified exec mirror its generic spawn
/// lifecycle into a resolved plugin-script execution.
///
/// Implementations must preserve the caller-provided callback order: unified
/// exec invokes the generic `SpawnLifecycle` first, then forwards the matching
/// plugin callback. Terminal callbacks may be repeated by cleanup paths, so
/// implementations must keep cancellation and finish handling idempotent.
trait PluginScriptLifecycle: Send + Sync {
    fn mark_started(&self);
    fn mark_cancelled(&self);
    fn finish(&self, exit_code: Option<i32>, failed: bool);
}

impl PluginScriptLifecycle for PluginScriptExecution {
    fn mark_started(&self) {
        self.mark_started();
    }

    fn mark_cancelled(&self) {
        self.mark_cancelled();
    }

    fn finish(&self, exit_code: Option<i32>, failed: bool) {
        self.finish(exit_code, failed);
    }
}

/// Adds plugin attribution without replacing unified exec's generic spawn work.
struct PluginScriptSpawnLifecycle {
    inner: SpawnLifecycleHandle,
    plugin_script: Arc<dyn PluginScriptLifecycle>,
}

impl std::fmt::Debug for PluginScriptSpawnLifecycle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PluginScriptSpawnLifecycle")
            .field("inner", &self.inner)
            .field("plugin_script", &"tracked")
            .finish()
    }
}

impl SpawnLifecycle for PluginScriptSpawnLifecycle {
    #[cfg(unix)]
    fn inherited_fds(&self) -> Vec<i32> {
        self.inner.inherited_fds()
    }

    fn after_spawn(&mut self) {
        self.inner.after_spawn();
        self.plugin_script.mark_started();
    }

    fn mark_cancelled(&self) {
        self.inner.mark_cancelled();
        self.plugin_script.mark_cancelled();
    }

    fn finish(&self, exit_code: Option<i32>, failed: bool) {
        self.inner.finish(exit_code, failed);
        self.plugin_script.finish(exit_code, failed);
    }
}

impl Drop for PluginScriptSpawnLifecycle {
    fn drop(&mut self) {
        self.plugin_script
            .finish(/*exit_code*/ None, /*failed*/ true);
    }
}

fn wrap_plugin_script_lifecycle(
    inner: SpawnLifecycleHandle,
    plugin_script: Option<Arc<dyn PluginScriptLifecycle>>,
) -> SpawnLifecycleHandle {
    match plugin_script {
        Some(plugin_script) => Box::new(PluginScriptSpawnLifecycle {
            inner,
            plugin_script,
        }),
        None => inner,
    }
}

fn wrap_spawn_lifecycle(
    inner: SpawnLifecycleHandle,
    plugin_script: Option<&Arc<PluginScriptExecution>>,
) -> SpawnLifecycleHandle {
    wrap_plugin_script_lifecycle(
        inner,
        plugin_script
            .map(|plugin_script| Arc::clone(plugin_script) as Arc<dyn PluginScriptLifecycle>),
    )
}

fn should_track_plugin_script(req: &UnifiedExecRequest) -> bool {
    !req.turn_environment.environment.is_remote()
}

fn resolve_plugin_script_execution(
    req: &UnifiedExecRequest,
    ctx: &ToolCtx,
) -> Option<Arc<PluginScriptExecution>> {
    let cwd = req.cwd.to_abs_path().ok()?;
    should_track_plugin_script(req)
        .then(|| {
            PluginScriptExecution::resolve(
                ctx.session.as_ref(),
                ctx.turn.as_ref(),
                &req.hook_command,
                &cwd,
                req.shell_type,
            )
        })
        .flatten()
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
    type ApprovalKey = UnifiedExecApprovalKey;

    fn approval_keys(&self, req: &UnifiedExecRequest) -> Vec<Self::ApprovalKey> {
        vec![UnifiedExecApprovalKey {
            environment_id: req.turn_environment.environment_id.clone(),
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
        let session = ctx.session;
        let turn = ctx.turn;
        let call_id = ctx.call_id.to_string();
        let command = req.command.clone();
        let environment_id = Some(req.turn_environment.environment_id.clone());
        let retry_reason = ctx.retry_reason.clone();
        let reason = retry_reason.clone().or_else(|| req.justification.clone());
        let guardian_review_id = ctx.guardian_review_id.clone();
        Box::pin(async move {
            let native_cwd = match req.cwd.to_abs_path() {
                Ok(c) => c,
                Err(e) => {
                    // TODO(anp) make sandboxing work for foreign OSes, in the meantime this should
                    // be impossible for single-OS app-servers
                    error!(cwd = %req.cwd, ?e, "got non-native path in start_approval_async");
                    return ReviewDecision::Abort;
                }
            };
            if let Some(review_id) = guardian_review_id {
                return review_approval_request(
                    session,
                    turn,
                    review_id,
                    GuardianApprovalRequest::ExecCommand {
                        id: call_id,
                        command,
                        cwd: native_cwd.clone(),
                        sandbox_permissions: req.sandbox_permissions,
                        additional_permissions: req.additional_permissions.clone(),
                        justification: req.justification.clone(),
                        tty: req.tty,
                    },
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
                        environment_id,
                        command,
                        native_cwd,
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
            req.justification.clone(),
        ))
    }

    fn sandbox_permissions(&self, req: &UnifiedExecRequest) -> SandboxPermissions {
        req.sandbox_permissions
    }
}

impl<'a> ToolRuntime<UnifiedExecRequest, UnifiedExecProcess> for UnifiedExecRuntime<'a> {
    fn sandbox_cwd<'b>(&self, req: &'b UnifiedExecRequest) -> Option<&'b PathUri> {
        Some(&req.sandbox_cwd)
    }

    fn network_approval_spec(
        &self,
        req: &UnifiedExecRequest,
        ctx: &ToolCtx,
    ) -> Option<NetworkApprovalSpec> {
        let file_system_sandbox_policy = ctx.turn.file_system_sandbox_policy();
        let sandbox_permissions = sandbox_permissions_preserving_denied_reads(
            req.sandbox_permissions,
            &file_system_sandbox_policy,
        );
        let network =
            managed_network_for_sandbox_permissions(req.network.as_ref(), sandbox_permissions)?;
        Some(NetworkApprovalSpec {
            network: Some(network.clone()),
            mode: NetworkApprovalMode::Deferred,
            trigger: GuardianNetworkAccessTrigger {
                call_id: ctx.call_id.clone(),
                tool_name: flat_tool_name(&ctx.tool_name).into_owned(),
                command: req.command.clone(),
                cwd: req.cwd.to_abs_path().ok()?,
                sandbox_permissions: req.sandbox_permissions,
                additional_permissions: req.additional_permissions.clone(),
                justification: req.justification.clone(),
                tty: Some(req.tty),
            },
            command: req.hook_command.clone(),
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
        let shell = req
            .turn_environment
            .shell
            .as_ref()
            .unwrap_or(session_shell.as_ref());
        let environment_is_remote = req.turn_environment.environment.is_remote();
        let shell_snapshot_location = if environment_is_remote {
            None
        } else {
            // TODO(anp): Make shell snapshot lookup accept PathUri.
            let native_cwd = req
                .cwd
                .to_abs_path()
                .map_err(|err| ToolError::Rejected(err.to_string()))?;
            req.turn_environment.shell_snapshot(&native_cwd)
        };
        let (file_system_sandbox_policy, _) = attempt.permissions.to_runtime_permissions();
        let launch_sandbox_permissions = sandbox_permissions_preserving_denied_reads(
            req.sandbox_permissions,
            &file_system_sandbox_policy,
        );
        let managed_network = managed_network_for_sandbox_permissions(
            req.network.as_ref(),
            launch_sandbox_permissions,
        );
        let mut env = exec_env_for_sandbox_permissions(&req.env, launch_sandbox_permissions);
        if let Some(network) = managed_network {
            network.apply_to_env(&mut env);
        }
        let environment_is_remote = req.turn_environment.environment.is_remote();
        // Resolve before snapshot, sandbox, PowerShell, or exec-server rewriting:
        // only the original local request has safe plugin roots and cwd.
        let plugin_script = resolve_plugin_script_execution(req, ctx);
        let explicit_env_overrides = req.explicit_env_overrides.clone();
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
        let command = if environment_is_remote {
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
        let command = disable_powershell_profile_for_elevated_windows_sandbox(
            &command,
            Some(&req.shell_type),
            attempt.sandbox,
            attempt.windows_sandbox_level,
        );
        let command = if matches!(req.shell_type, ShellType::PowerShell) {
            prefix_powershell_script_with_utf8(&command)
        } else {
            command
        };

        if let UnifiedExecShellMode::ZshFork(zsh_fork_config) = &self.shell_mode {
            let command = build_unified_exec_sandbox_command(
                &command,
                &req.cwd,
                &env,
                req.additional_permissions.clone(),
            )
            .map_err(|error| match error {
                ToolError::Rejected(_) => {
                    ToolError::Rejected("missing command line for PTY".to_string())
                }
                error @ ToolError::Codex(_) => error,
            })?;
            let options = unified_exec_options(attempt.network_denial_cancellation_token.clone());
            let mut exec_env = attempt
                .env_for(command, options, managed_network)
                .map_err(ToolError::Codex)?;
            exec_env.exec_server_env_config = req.exec_server_env_config.clone();
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
                    if req.turn_environment.environment.is_remote() {
                        return Err(ToolError::Rejected(
                            "unified_exec zsh-fork is not supported for remote environments"
                                .to_string(),
                        ));
                    }
                    return self
                        .manager
                        .open_session_with_exec_env(
                            req.process_id,
                            &prepared.exec_request,
                            req.tty,
                            wrap_spawn_lifecycle(prepared.spawn_lifecycle, plugin_script.as_ref()),
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
            req.additional_permissions.clone(),
        )
        .map_err(|error| match error {
            ToolError::Rejected(_) => {
                ToolError::Rejected("missing command line for PTY".to_string())
            }
            error @ ToolError::Codex(_) => error,
        })?;
        let options = unified_exec_options(attempt.network_denial_cancellation_token.clone());
        let mut exec_env = attempt
            .env_for(command, options, managed_network)
            .map_err(ToolError::Codex)?;
        exec_env.exec_server_env_config = req.exec_server_env_config.clone();
        let spawn_lifecycle =
            wrap_spawn_lifecycle(Box::new(NoopSpawnLifecycle), plugin_script.as_ref());
        self.manager
            .open_session_with_exec_env(
                req.process_id,
                &exec_env,
                req.tty,
                spawn_lifecycle,
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
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::DEFAULT_EXEC_COMMAND_TIMEOUT_MS;
    use crate::tools::sandboxing::ToolRuntime;
    use codex_exec_server::Environment;
    use codex_exec_server::LOCAL_ENVIRONMENT_ID;
    use codex_tools::ZshForkConfig;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use codex_utils_path_uri::PathUri;
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::tempdir;

    fn test_turn_environment(cwd: PathUri) -> TurnEnvironment {
        TurnEnvironment::new(
            LOCAL_ENVIRONMENT_ID.to_string(),
            Arc::new(Environment::default_for_tests()),
            cwd,
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
        request.turn_environment.environment_id = "remote".to_string();
        let original_key = runtime.approval_keys(&request);
        request.turn_environment.environment_id = "other".to_string();
        let other_key = runtime.approval_keys(&request);

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

    #[derive(Debug)]
    struct RecordingLifecycle {
        #[cfg(unix)]
        inherited_fds: Vec<i32>,
        events: Arc<std::sync::Mutex<Vec<&'static str>>>,
    }

    impl SpawnLifecycle for RecordingLifecycle {
        #[cfg(unix)]
        fn inherited_fds(&self) -> Vec<i32> {
            self.inherited_fds.clone()
        }

        fn after_spawn(&mut self) {
            self.events.lock().unwrap().push("inner_started");
        }

        fn mark_cancelled(&self) {
            self.events.lock().unwrap().push("inner_cancelled");
        }

        fn finish(&self, _exit_code: Option<i32>, _failed: bool) {
            self.events.lock().unwrap().push("inner_finished");
        }
    }

    struct RecordingPluginLifecycle {
        events: Arc<std::sync::Mutex<Vec<&'static str>>>,
        terminal_emitted: std::sync::atomic::AtomicBool,
    }

    impl PluginScriptLifecycle for RecordingPluginLifecycle {
        fn mark_started(&self) {
            self.events.lock().unwrap().push("plugin_started");
        }

        fn mark_cancelled(&self) {
            self.events.lock().unwrap().push("plugin_cancelled");
        }

        fn finish(&self, _exit_code: Option<i32>, _failed: bool) {
            if !self
                .terminal_emitted
                .swap(true, std::sync::atomic::Ordering::AcqRel)
            {
                self.events.lock().unwrap().push("plugin_finished");
            }
        }
    }

    fn recording_lifecycle() -> (
        PluginScriptSpawnLifecycle,
        Arc<std::sync::Mutex<Vec<&'static str>>>,
    ) {
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let lifecycle = PluginScriptSpawnLifecycle {
            inner: Box::new(RecordingLifecycle {
                #[cfg(unix)]
                inherited_fds: vec![7, 11],
                events: Arc::clone(&events),
            }),
            plugin_script: Arc::new(RecordingPluginLifecycle {
                events: Arc::clone(&events),
                terminal_emitted: std::sync::atomic::AtomicBool::new(false),
            }),
        };
        (lifecycle, events)
    }

    #[test]
    fn plugin_lifecycle_starts_after_inner() {
        let (mut lifecycle, events) = recording_lifecycle();

        lifecycle.after_spawn();

        assert_eq!(
            *events.lock().unwrap(),
            vec!["inner_started", "plugin_started"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn plugin_lifecycle_preserves_inherited_fds() {
        let (lifecycle, _events) = recording_lifecycle();

        assert_eq!(lifecycle.inherited_fds(), vec![7, 11]);
    }

    #[test]
    fn plugin_lifecycle_wraps_local_shell_trackers() {
        for shell_type in [ShellType::PowerShell, ShellType::Cmd, ShellType::Bash] {
            let events = Arc::new(std::sync::Mutex::new(Vec::new()));
            let mut lifecycle = wrap_plugin_script_lifecycle(
                Box::new(RecordingLifecycle {
                    #[cfg(unix)]
                    inherited_fds: Vec::new(),
                    events: Arc::clone(&events),
                }),
                Some(Arc::new(RecordingPluginLifecycle {
                    events: Arc::clone(&events),
                    terminal_emitted: std::sync::atomic::AtomicBool::new(false),
                })),
            );

            lifecycle.after_spawn();
            lifecycle.finish(Some(0), /*failed*/ false);
            assert_eq!(
                *events.lock().unwrap(),
                vec![
                    "inner_started",
                    "plugin_started",
                    "inner_finished",
                    "plugin_finished"
                ],
                "{shell_type:?} should keep plugin attribution",
            );
        }
    }

    #[test]
    fn plugin_lifecycle_forwards_cancellation_and_idempotent_terminal_state() {
        let (lifecycle, events) = recording_lifecycle();

        lifecycle.mark_cancelled();
        lifecycle.finish(Some(0), /*failed*/ false);
        lifecycle.finish(Some(9), /*failed*/ true);
        drop(lifecycle);

        assert_eq!(
            *events.lock().unwrap(),
            vec![
                "inner_cancelled",
                "plugin_cancelled",
                "inner_finished",
                "plugin_finished",
                "inner_finished",
            ]
        );
    }

    #[tokio::test]
    async fn remote_unified_exec_requests_do_not_track_plugin_scripts() {
        let mut request = test_request(
            SandboxPermissions::UseDefault,
            ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
        );
        request.turn_environment.environment = Arc::new(
            Environment::create_for_tests(Some("ws://127.0.0.1:1/remote-exec-server".to_string()))
                .expect("remote environment"),
        );

        assert!(!should_track_plugin_script(&request));
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
