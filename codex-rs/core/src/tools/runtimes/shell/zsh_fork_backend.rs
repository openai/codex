use super::ShellRequest;
use crate::sandboxing::ExecRequest;
use crate::tools::runtimes::unified_exec::UnifiedExecRequest;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::unified_exec::SpawnLifecycleHandle;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_tools::ZshForkConfig;

pub(crate) struct PreparedUnifiedExecSpawn {
    pub(crate) exec_request: ExecRequest,
    pub(crate) spawn_lifecycle: SpawnLifecycleHandle,
}

/// Runs the zsh-fork shell-command backend when this request should be handled
/// by executable-level escalation instead of the default shell runtime.
///
/// Returns `Ok(None)` when the current platform or request shape should fall
/// back to the normal shell-command path.
pub(crate) async fn maybe_run_shell_command(
    req: &ShellRequest,
    attempt: &SandboxAttempt<'_>,
    ctx: &ToolCtx,
    command: &[String],
) -> Result<Option<ExecToolCallOutput>, ToolError> {
    imp::maybe_run_shell_command(req, attempt, ctx, command).await
}

/// Prepares unified exec to launch through the zsh-fork backend when the
/// request matches a wrapped `zsh -c/-lc` command on a supported platform.
///
/// Returns the transformed `ExecRequest` plus a spawn lifecycle that keeps the
/// escalation server alive for the session and performs post-spawn cleanup.
/// Returns `Ok(None)` when unified exec should use its normal spawn path.
pub(crate) async fn maybe_prepare_unified_exec(
    req: &UnifiedExecRequest,
    attempt: &SandboxAttempt<'_>,
    ctx: &ToolCtx,
    exec_request: ExecRequest,
    zsh_fork_config: &ZshForkConfig,
) -> Result<Option<PreparedUnifiedExecSpawn>, ToolError> {
    imp::maybe_prepare_unified_exec(req, attempt, ctx, exec_request, zsh_fork_config).await
}

#[cfg(unix)]
mod imp {
    use super::*;
    use crate::tools::runtimes::shell::unix_escalation;
    use crate::unified_exec::SpawnLifecycle;
    use codex_shell_escalation::ESCALATE_SOCKET_ENV_VAR;
    use codex_shell_escalation::EscalationSession;

    #[derive(Debug)]
    struct ZshForkSpawnLifecycle {
        escalation_session: EscalationSession,
    }

    impl SpawnLifecycle for ZshForkSpawnLifecycle {
        fn inherited_fds(&self) -> Vec<i32> {
            self.escalation_session
                .env()
                .get(ESCALATE_SOCKET_ENV_VAR)
                .and_then(|fd| fd.parse().ok())
                .into_iter()
                .collect()
        }

        fn after_spawn(&mut self) {
            self.escalation_session.close_client_socket();
        }
    }

    pub(super) async fn maybe_run_shell_command(
        req: &ShellRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
        command: &[String],
    ) -> Result<Option<ExecToolCallOutput>, ToolError> {
        unix_escalation::try_run_zsh_fork(req, attempt, ctx, command).await
    }

    pub(super) async fn maybe_prepare_unified_exec(
        req: &UnifiedExecRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
        exec_request: ExecRequest,
        zsh_fork_config: &ZshForkConfig,
    ) -> Result<Option<PreparedUnifiedExecSpawn>, ToolError> {
        let Some(prepared) = unix_escalation::prepare_unified_exec_zsh_fork(
            req,
            attempt,
            ctx,
            exec_request,
            zsh_fork_config.shell_zsh_path.as_path(),
            zsh_fork_config.main_execve_wrapper_exe.as_path(),
        )
        .await?
        else {
            return Ok(None);
        };

        Ok(Some(PreparedUnifiedExecSpawn {
            exec_request: prepared.exec_request,
            spawn_lifecycle: Box::new(ZshForkSpawnLifecycle {
                escalation_session: prepared.escalation_session,
            }),
        }))
    }
}

#[cfg(not(unix))]
mod imp {
    use super::*;

    pub(super) async fn maybe_run_shell_command(
        req: &ShellRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
        command: &[String],
    ) -> Result<Option<ExecToolCallOutput>, ToolError> {
        let _ = (req, attempt, ctx, command);
        Ok(None)
    }

    pub(super) async fn maybe_prepare_unified_exec(
        req: &UnifiedExecRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx,
        exec_request: ExecRequest,
        zsh_fork_config: &ZshForkConfig,
    ) -> Result<Option<PreparedUnifiedExecSpawn>, ToolError> {
        let _ = (req, attempt, ctx, exec_request, zsh_fork_config);
        Ok(None)
    }
}
