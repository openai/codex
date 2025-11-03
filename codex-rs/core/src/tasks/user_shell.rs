use std::sync::Arc;

use async_trait::async_trait;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;
use tracing::error;
use uuid::Uuid;

use crate::codex::TurnContext;
use crate::exec::SandboxType;
use crate::exec::StdoutStream;
use crate::exec::execute_exec_env;
use crate::exec_env::create_env;
use crate::protocol::EventMsg;
use crate::protocol::TaskStartedEvent;
use crate::sandboxing::SandboxManager;
use crate::state::TaskKind;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::runtimes::build_command_spec;
use crate::tools::sandboxing::ToolError;

use super::SessionTask;
use super::SessionTaskContext;

#[derive(Clone)]
pub(crate) struct UserShellCommandTask {
    command: String,
}

impl UserShellCommandTask {
    pub(crate) fn new(command: String) -> Self {
        Self { command }
    }
}

#[async_trait]
impl SessionTask for UserShellCommandTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        turn_context: Arc<TurnContext>,
        _input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        let event = EventMsg::TaskStarted(TaskStartedEvent {
            model_context_window: turn_context.client.get_model_context_window(),
        });
        let session = session.clone_session();
        session.send_event(turn_context.as_ref(), event).await;

        // Execute the user's script under their default shell when known; this
        // allows commands that use shell features (pipes, &&, redirects, etc.).
        // We do not source rc files or otherwise reformat the script.
        let shell_invocation = match session.user_shell() {
            crate::shell::Shell::Zsh(zsh) => vec![
                zsh.shell_path.clone(),
                "-lc".to_string(),
                self.command.clone(),
            ],
            crate::shell::Shell::Bash(bash) => vec![
                bash.shell_path.clone(),
                "-lc".to_string(),
                self.command.clone(),
            ],
            crate::shell::Shell::PowerShell(ps) => vec![
                ps.exe.clone(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                self.command.clone(),
            ],
            crate::shell::Shell::Unknown => {
                shlex::split(&self.command).unwrap_or_else(|| vec![self.command.clone()])
            }
        };

        if cancellation_token.is_cancelled() {
            return None;
        }

        let call_id = Uuid::new_v4().to_string();
        let emitter = ToolEmitter::shell(shell_invocation.clone(), turn_context.cwd.clone(), true);
        let event_ctx = ToolEventCtx::new(session.as_ref(), turn_context.as_ref(), &call_id, None);
        emitter.begin(event_ctx).await;

        let env = create_env(&turn_context.shell_environment_policy);
        let spec = match build_command_spec(
            &shell_invocation,
            &turn_context.cwd,
            &env,
            None,
            None,
            None,
        ) {
            Ok(spec) => spec,
            Err(err) => {
                let event_ctx =
                    ToolEventCtx::new(session.as_ref(), turn_context.as_ref(), &call_id, None);
                if let Err(finish_err) = emitter.finish(event_ctx, Err(err)).await {
                    error!(?finish_err, "user shell command failed: {finish_err:?}");
                }
                return None;
            }
        };

        let manager = SandboxManager::new();
        let exec_env = match manager.transform(
            &spec,
            &turn_context.sandbox_policy,
            SandboxType::None,
            &turn_context.cwd,
            turn_context.codex_linux_sandbox_exe.as_ref(),
        ) {
            Ok(env) => env,
            Err(err) => {
                let event_ctx =
                    ToolEventCtx::new(session.as_ref(), turn_context.as_ref(), &call_id, None);
                let tool_error = ToolError::Codex(err.into());
                if let Err(finish_err) = emitter.finish(event_ctx, Err(tool_error)).await {
                    error!(?finish_err, "user shell command failed: {finish_err:?}");
                }
                return None;
            }
        };

        let stdout_stream = StdoutStream {
            sub_id: turn_context.sub_id.clone(),
            call_id: call_id.clone(),
            tx_event: session.get_tx_event(),
        };

        let exec_result = tokio::select! {
            _ = cancellation_token.cancelled() => None,
            res = execute_exec_env(exec_env, &turn_context.sandbox_policy, Some(stdout_stream)) => Some(res),
        };

        if let Some(result) = exec_result {
            let event_ctx =
                ToolEventCtx::new(session.as_ref(), turn_context.as_ref(), &call_id, None);
            if let Err(err) = emitter
                .finish(event_ctx, result.map_err(ToolError::Codex))
                .await
            {
                error!(?err, "user shell command failed: {err:?}");
            }
        }
        None
    }
}
