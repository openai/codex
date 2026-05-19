use std::sync::Arc;

use codex_protocol::openai_models::ConfigShellToolType;
use codex_tools::ShellCommandBackendConfig;
use codex_tools::ToolEnvironmentMode;

use crate::tools::handlers::ExecCommandHandler;
use crate::tools::handlers::ExecCommandHandlerOptions;
use crate::tools::handlers::ShellCommandHandler;
use crate::tools::handlers::ShellCommandHandlerOptions;
use crate::tools::handlers::WriteStdinHandler;
use crate::tools::registry::CoreToolRuntime;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShellToolsOptions {
    pub(crate) shell_type: ConfigShellToolType,
    pub(crate) shell_command_backend: ShellCommandBackendConfig,
    pub(crate) environment_mode: ToolEnvironmentMode,
    pub(crate) allow_login_shell: bool,
    pub(crate) exec_permission_approvals_enabled: bool,
}

pub(crate) fn build_shell_tools(options: ShellToolsOptions) -> Vec<Arc<dyn CoreToolRuntime>> {
    let mut runtimes = Vec::new();
    if !options.environment_mode.has_environment() {
        return runtimes;
    }

    let include_environment_id = matches!(options.environment_mode, ToolEnvironmentMode::Multiple);
    match options.shell_type {
        ConfigShellToolType::UnifiedExec => {
            add_runtime(
                &mut runtimes,
                ExecCommandHandler::new(ExecCommandHandlerOptions {
                    allow_login_shell: options.allow_login_shell,
                    exec_permission_approvals_enabled: options.exec_permission_approvals_enabled,
                    include_environment_id,
                }),
            );
            add_runtime(&mut runtimes, WriteStdinHandler);

            // Keep the legacy shell tool registered as a hidden runtime while
            // unified exec is model-visible.
            add_runtime(
                &mut runtimes,
                ShellCommandHandler::from(options.shell_command_backend),
            );
        }
        ConfigShellToolType::Disabled => {}
        ConfigShellToolType::Default
        | ConfigShellToolType::Local
        | ConfigShellToolType::ShellCommand => {
            add_runtime(
                &mut runtimes,
                ShellCommandHandler::new(ShellCommandHandlerOptions {
                    backend_config: options.shell_command_backend,
                    allow_login_shell: options.allow_login_shell,
                    exec_permission_approvals_enabled: options.exec_permission_approvals_enabled,
                }),
            );
        }
    }

    runtimes
}

fn add_runtime<T>(runtimes: &mut Vec<Arc<dyn CoreToolRuntime>>, runtime: T)
where
    T: CoreToolRuntime + 'static,
{
    runtimes.push(Arc::new(runtime));
}
