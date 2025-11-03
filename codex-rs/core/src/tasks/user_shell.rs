use std::sync::Arc;

use async_trait::async_trait;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;
use tracing::error;
use uuid::Uuid;

use crate::codex::TurnContext;
use crate::exec::SandboxType;
use crate::exec::StdoutStream;
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

        // Execute the user's script under their default shell when known. Use
        // execute_exec_env directly to avoid approvals/sandbox overhead.
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

        // Emit shell begin event to keep UI consistent with tool runs.
        let call_id = Uuid::new_v4().to_string();
        let emitter = ToolEmitter::shell(shell_invocation.clone(), turn_context.cwd.clone(), true);
        let event_ctx = ToolEventCtx::new(session.as_ref(), turn_context.as_ref(), &call_id, None);
        emitter.begin(event_ctx).await;

        let env = ExecEnv {
            command: shell_invocation,
            cwd: turn_context.cwd.clone(),
            env: create_env(&turn_context.shell_environment_policy),
            timeout_ms: None,
            sandbox: SandboxType::None,
            with_escalated_permissions: None,
            justification: None,
            arg0: None,
        };
        let stream = Some(StdoutStream {
            sub_id: turn_context.sub_id.clone(),
            call_id: call_id.clone(),
            tx_event: session.get_tx_event(),
        });

        let policy = turn_context.sandbox_policy.clone();
        let mut exec_task = tokio::spawn(async move {
            crate::exec::execute_exec_env(env, &policy, stream).await
        });

        tokio::select! {
            res = &mut exec_task => {
                match res {
                    Ok(Ok(output)) => {
                        let event_ctx = ToolEventCtx::new(session.as_ref(), turn_context.as_ref(), &call_id, None);
                        let _ = emitter.finish(event_ctx, Ok(output)).await;
                    }
                    Ok(Err(err)) => {
                        let event_ctx = ToolEventCtx::new(session.as_ref(), turn_context.as_ref(), &call_id, None);
                        let _ = emitter.finish(event_ctx, Err(ToolError::Codex(err))).await;
                    }
                    Err(join_err) => {
                        error!("user shell exec task join error: {join_err:?}");
                    }
                }
            }
            _ = cancellation_token.cancelled() => {
                exec_task.abort();
                // Session will emit TurnAborted; do not finish emitter to avoid
                // emitting ExecCommandEnd for an interrupted run.
            }
        }

        None
    }
}
