#[cfg(unix)]
pub mod unix;

#[cfg(unix)]
pub use unix::*;

#[cfg(unix)]
pub use unix::escalate_client::run;
#[cfg(unix)]
pub use unix::escalate_protocol::EscalateAction;
#[cfg(unix)]
pub use unix::escalate_server::EscalationPolicyFactory;
#[cfg(unix)]
pub use unix::escalate_server::ExecParams;
#[cfg(unix)]
pub use unix::escalate_server::ExecResult;
#[cfg(unix)]
pub use unix::escalation_policy::EscalationPolicy;
#[cfg(unix)]
pub use unix::stopwatch::Stopwatch;

#[cfg(unix)]
mod legacy_api {
    use std::collections::HashMap;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use codex_execpolicy::Policy;
    use codex_protocol::config_types::WindowsSandboxLevel;
    use codex_protocol::models::SandboxPermissions as ProtocolSandboxPermissions;
    use tokio::sync::RwLock;
    use tokio_util::sync::CancellationToken;

    use crate::unix::escalate_server::EscalationPolicyFactory;
    use crate::unix::escalate_server::ExecParams;
    use crate::unix::escalate_server::ExecResult;
    use crate::unix::escalate_server::SandboxState;
    use crate::unix::escalate_server::ShellCommandExecutor;

    struct CoreShellCommandExecutor;

    #[async_trait::async_trait]
    impl ShellCommandExecutor for CoreShellCommandExecutor {
        async fn run(
            &self,
            command: Vec<String>,
            cwd: PathBuf,
            env: HashMap<String, String>,
            cancel_rx: CancellationToken,
            sandbox_state: &SandboxState,
        ) -> anyhow::Result<ExecResult> {
            let result = codex_core::exec::process_exec_tool_call(
                codex_core::exec::ExecParams {
                    command,
                    cwd,
                    expiration: codex_core::exec::ExecExpiration::Cancellation(cancel_rx),
                    env,
                    network: None,
                    sandbox_permissions: ProtocolSandboxPermissions::UseDefault,
                    windows_sandbox_level: WindowsSandboxLevel::Disabled,
                    justification: None,
                    arg0: None,
                },
                &sandbox_state.sandbox_policy,
                &sandbox_state.sandbox_cwd,
                &sandbox_state.codex_linux_sandbox_exe,
                sandbox_state.use_linux_sandbox_bwrap,
                None,
            )
            .await?;

            Ok(ExecResult {
                exit_code: result.exit_code,
                output: result.aggregated_output.text,
                duration: result.duration,
                timed_out: result.timed_out,
            })
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run_escalate_server(
        exec_params: ExecParams,
        sandbox_state: &codex_core::SandboxState,
        shell_program: impl AsRef<Path>,
        execve_wrapper: impl AsRef<Path>,
        policy: Arc<RwLock<Policy>>,
        escalation_policy_factory: impl EscalationPolicyFactory,
        effective_timeout: Duration,
    ) -> anyhow::Result<ExecResult> {
        let sandbox_state = SandboxState {
            sandbox_policy: sandbox_state.sandbox_policy.clone(),
            codex_linux_sandbox_exe: sandbox_state.codex_linux_sandbox_exe.clone(),
            sandbox_cwd: sandbox_state.sandbox_cwd.clone(),
            use_linux_sandbox_bwrap: sandbox_state.use_linux_sandbox_bwrap,
        };
        crate::unix::escalate_server::run_escalate_server(
            exec_params,
            &sandbox_state,
            shell_program,
            execve_wrapper,
            policy,
            escalation_policy_factory,
            effective_timeout,
            &CoreShellCommandExecutor,
        )
        .await
    }
}

#[cfg(unix)]
pub use legacy_api::run_escalate_server;
