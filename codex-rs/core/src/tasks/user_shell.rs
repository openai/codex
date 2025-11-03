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
use crate::sandboxing::ExecEnv;
use crate::state::TaskKind;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
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

        let command = shell_invocation;
        let cwd = turn_context.resolve_path(None);
        let call_id = Uuid::new_v4().to_string();
        let emitter = ToolEmitter::shell(command.clone(), cwd.clone(), true);
        let event_ctx = ToolEventCtx::new(session.as_ref(), turn_context.as_ref(), &call_id, None);
        emitter.begin(event_ctx).await;

        let exec_env = ExecEnv {
            command,
            cwd: cwd.clone(),
            env: create_env(&turn_context.shell_environment_policy),
            timeout_ms: None,
            sandbox: SandboxType::None,
            with_escalated_permissions: None,
            justification: None,
            arg0: None,
        };

        let stdout_stream = StdoutStream {
            sub_id: turn_context.sub_id.clone(),
            call_id: call_id.clone(),
            tx_event: session.get_tx_event(),
        };

        let exec_result = tokio::select! {
            _ = cancellation_token.cancelled() => {
                return None;
            }
            res = execute_exec_env(exec_env, &turn_context.sandbox_policy, Some(stdout_stream)) => res,
        };

        let event_ctx = ToolEventCtx::new(session.as_ref(), turn_context.as_ref(), &call_id, None);
        if let Err(err) = emitter
            .finish(event_ctx, exec_result.map_err(ToolError::Codex))
            .await
        {
            error!("user shell command failed: {err:?}");
        }
        None
    }
}
