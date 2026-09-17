use std::collections::HashMap;
use std::sync::Arc;

use codex_exec_server::ExecParams;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::ShellInfo;
use codex_exec_server::ShellSnapshotRequest;
use codex_features::Feature;
use codex_protocol::protocol::AskForApproval;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxablePreference;
use codex_tools::ToolName;
use codex_tools::UnifiedExecShellMode;
use codex_utils_path_uri::PathUri;
use tokio_util::task::AbortOnDropHandle;
use uuid::Uuid;

use super::ExecCommandRequest;
use super::UnifiedExecContext;
use super::process_manager::apply_unified_exec_env;
use super::process_manager::exec_env_policy_from_shell_policy;
use crate::config::NetworkProxySpec;
use crate::exec_env::CODEX_THREAD_ID_ENV_VAR;
use crate::exec_env::inject_apply_patch_env;
use crate::exec_env::inject_permission_profile_env;
use crate::exec_env::inject_session_env;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::shell::ShellType;
use crate::tools::sandboxing::executor_windows_sandbox_selection;

impl Session {
    pub(crate) fn prewarm_shell_snapshots(
        self: &Arc<Self>,
        step_context: &StepContext,
    ) -> Option<AbortOnDropHandle<()>> {
        if !self.features().enabled(Feature::ShellSnapshotV2)
            || step_context
                .tool_router
                .tool_runtime(&ToolName::plain("exec_command"))
                .is_none()
        {
            return None;
        }

        let config = Arc::clone(&step_context.turn.config);
        let user_shell = self.user_shell();
        if config.active_project.is_untrusted()
            || matches!(
                step_context.settings.approval_policy(),
                AskForApproval::UnlessTrusted
            )
            || step_context.turn.network.is_some()
            || config
                .permissions
                .network
                .as_ref()
                .is_some_and(NetworkProxySpec::enabled)
            || !matches!(
                step_context.turn.unified_exec_shell_mode,
                UnifiedExecShellMode::Direct
            )
        {
            return None;
        }

        let environments = step_context.environments.clone();
        let session = Arc::clone(self);
        Some(AbortOnDropHandle::new(tokio::spawn(async move {
            for environment in environments.turn_environments() {
                // Remote starts cannot cancel an in-flight capture. Leave them, deferred
                // environments, and managed networking on the normal lazy capture path.
                if environment.environment.is_remote()
                    || !environment.shell_snapshot_v2_supported
                    || !environment.config().allow_login_shell
                    || environment.config().network_policy.is_some()
                {
                    continue;
                }

                let shell = environment.shell.as_ref().unwrap_or(user_shell.as_ref());
                if !matches!(
                    shell.shell_type,
                    ShellType::Bash | ShellType::Zsh | ShellType::Sh
                ) {
                    continue;
                }
                let sandbox = SandboxManager::new()
                    .should_sandbox(
                        environment.permission_profile(),
                        SandboxablePreference::Auto,
                        /*has_managed_network_requirements*/ false,
                    )
                    .then(|| FileSystemSandboxContext {
                        permissions: environment.permission_profile().clone(),
                        cwd: environment.cwd().clone(),
                        workspace_roots: environment.workspace_roots().to_vec(),
                        user_home_dir: None,
                        temporary_directories: None,
                        windows_sandbox_selection: executor_windows_sandbox_selection(
                            environment.config().windows_sandbox_level,
                            environment.cwd(),
                        ),
                        windows_sandbox_private_desktop: environment
                            .config()
                            .windows_sandbox_private_desktop,
                        windows_sandbox_proxy_settings_mode: Some(
                            session.windows_sandbox_proxy_settings_mode,
                        ),
                        use_legacy_landlock: environment.config().use_legacy_landlock,
                    });
                let mut env = apply_unified_exec_env(HashMap::new());
                env.insert(
                    CODEX_THREAD_ID_ENV_VAR.to_string(),
                    session.thread_id().to_string(),
                );
                inject_session_env(&mut env, session.session_id());
                inject_apply_patch_env(&mut env, &config.features);
                inject_permission_profile_env(
                    &mut env,
                    environment.active_permission_profile().as_ref(),
                );
                #[cfg(unix)]
                if let Some(path) = environment.shell_environment_policy().r#set.get("PATH") {
                    env.insert("PATH".to_string(), path.clone());
                    crate::tools::runtimes::apply_package_path_prepend(
                        &mut env,
                        &mut crate::tools::runtimes::RuntimePathPrepends::default(),
                    );
                }
                let params = ExecParams {
                    process_id: format!("shell-snapshot-warmup-{}", Uuid::new_v4()).into(),
                    metadata: None,
                    // This capture-only operation never starts the command or routes
                    // speculative work through tool approvals or exec-policy bypass.
                    argv: shell.derive_exec_args("", /*use_login_shell*/ true),
                    cwd: environment.cwd().clone(),
                    env_policy: Some(exec_env_policy_from_shell_policy(
                        environment.shell_environment_policy(),
                    )),
                    shell_snapshot: Some(ShellSnapshotRequest {
                        scope_id: format!(
                            "{}:{}",
                            session.thread_id(),
                            environment.selection.environment_id
                        ),
                        shell: ShellInfo {
                            name: shell.name().to_string(),
                            path: shell.shell_path.to_string_lossy().into_owned(),
                        },
                    }),
                    env,
                    tty: false,
                    pipe_stdin: false,
                    arg0: None,
                    sandbox,
                    enforce_managed_network: false,
                    managed_network: None,
                    network_proxy: None,
                };

                let result = environment
                    .environment
                    .get_exec_backend()
                    .prewarm_shell_snapshot(params)
                    .await;
                if let Err(err) = result {
                    tracing::debug!("failed to warm shell snapshot: {err}");
                }
            }
        })))
    }
}

pub(super) fn shell_snapshot_request(
    request: &ExecCommandRequest,
    cwd: &PathUri,
    context: &UnifiedExecContext,
) -> Option<ShellSnapshotRequest> {
    if !context.session.features().enabled(Feature::ShellSnapshotV2)
        || !request.turn_environment.shell_snapshot_v2_supported
        || request.turn_environment.selection.cwd != *cwd
        || !matches!(request.shell_mode, UnifiedExecShellMode::Direct)
        || !matches!(
            request.shell_type,
            ShellType::Bash | ShellType::Zsh | ShellType::Sh
        )
        || request.command.get(1).is_none_or(|flag| flag != "-lc")
    {
        return None;
    }

    Some(ShellSnapshotRequest {
        scope_id: format!(
            "{}:{}",
            context.session.thread_id(),
            request.turn_environment.selection.environment_id
        ),
        shell: ShellInfo {
            name: request.shell_type.name().to_string(),
            path: request.command.first()?.clone(),
        },
    })
}
