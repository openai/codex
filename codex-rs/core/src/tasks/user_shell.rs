use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;
use tracing::error;
use uuid::Uuid;

use crate::codex::TurnContext;
use crate::exec::DeltaEventBuilder;
use crate::exec::ExecToolCallOutput;
use crate::exec::SandboxType;
use crate::exec::StdoutStream;
use crate::exec::StreamOutput;
use crate::exec::execute_exec_env;
use crate::exec_env::create_env;
use crate::parse_command::parse_command;
use crate::protocol::EventMsg;
use crate::protocol::SandboxPolicy;
use crate::protocol::TaskStartedEvent;
use crate::protocol::UserCommandBeginEvent;
use crate::protocol::UserCommandEndEvent;
use crate::sandboxing::ExecEnv;
use crate::state::TaskKind;
use crate::tools::format_exec_output_for_model;
use crate::tools::format_exec_output_str;

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

        fn build_user_message(text: String) -> ResponseItem {
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText { text }],
            }
        }

        let call_id = Uuid::new_v4().to_string();
        let raw_command = self.command.clone();
        let command_text = format!(
            "<user_shell_command>\n{raw_command}\n</user_shell_command>"
        );
        let command_items = [build_user_message(command_text)];
        session
            .record_conversation_items(turn_context.as_ref(), &command_items)
            .await;

        let parsed_cmd = parse_command(&shell_invocation);
        session
            .send_event(
                turn_context.as_ref(),
                EventMsg::UserCommandBegin(UserCommandBeginEvent {
                    call_id: call_id.clone(),
                    command: shell_invocation.clone(),
                    cwd: turn_context.cwd.clone(),
                    parsed_cmd,
                }),
            )
            .await;

        let exec_env = ExecEnv {
            command: shell_invocation,
            cwd: turn_context.cwd.clone(),
            env: create_env(&turn_context.shell_environment_policy),
            timeout_ms: None,
            sandbox: SandboxType::None,
            with_escalated_permissions: None,
            justification: None,
            arg0: None,
        };

        let stdout_stream = Some(StdoutStream {
            sub_id: turn_context.sub_id.clone(),
            call_id: call_id.clone(),
            tx_event: session.get_tx_event(),
        });

        let sandbox_policy = SandboxPolicy::DangerFullAccess;
        let exec_future = execute_exec_env(
            exec_env,
            &sandbox_policy,
            stdout_stream,
            Some(DeltaEventBuilder::user_command()),
        );
        tokio::pin!(exec_future);

        let exec_result = tokio::select! {
            res = &mut exec_future => Some(res),
            _ = cancellation_token.cancelled() => None,
        };

        match exec_result {
            None => {
                let aborted_message = "command aborted by user".to_string();
                let aborted_text =
                    format!("<user_shell_command_output>\n{aborted_message}\n</user_shell_command_output>");
                let output_items = [build_user_message(aborted_text)];
                session
                    .record_conversation_items(turn_context.as_ref(), &output_items)
                    .await;
                session
                    .send_event(
                        turn_context.as_ref(),
                        EventMsg::UserCommandEnd(UserCommandEndEvent {
                            call_id,
                            stdout: String::new(),
                            stderr: aborted_message.clone(),
                            aggregated_output: aborted_message.clone(),
                            exit_code: -1,
                            duration: Duration::ZERO,
                            formatted_output: aborted_message,
                        }),
                    )
                    .await;
            }
            Some(Ok(output)) => {
                session
                    .send_event(
                        turn_context.as_ref(),
                        EventMsg::UserCommandEnd(UserCommandEndEvent {
                            call_id: call_id.clone(),
                            stdout: output.stdout.text.clone(),
                            stderr: output.stderr.text.clone(),
                            aggregated_output: output.aggregated_output.text.clone(),
                            exit_code: output.exit_code,
                            duration: output.duration,
                            formatted_output: format_exec_output_str(&output),
                        }),
                    )
                    .await;

                let output_payload = format_exec_output_for_model(&output);
                let output_text =
                    format!("<user_shell_command_output>\n{output_payload}\n</user_shell_command_output>");
                let output_items = [build_user_message(output_text)];
                session
                    .record_conversation_items(turn_context.as_ref(), &output_items)
                    .await;
            }
            Some(Err(err)) => {
                error!("user shell command failed: {err:?}");
                let message = format!("execution error: {err:?}");
                let exec_output = ExecToolCallOutput {
                    exit_code: -1,
                    stdout: StreamOutput::new(String::new()),
                    stderr: StreamOutput::new(message.clone()),
                    aggregated_output: StreamOutput::new(message.clone()),
                    duration: Duration::ZERO,
                    timed_out: false,
                };
                session
                    .send_event(
                        turn_context.as_ref(),
                        EventMsg::UserCommandEnd(UserCommandEndEvent {
                            call_id,
                            stdout: exec_output.stdout.text.clone(),
                            stderr: exec_output.stderr.text.clone(),
                            aggregated_output: exec_output.aggregated_output.text.clone(),
                            exit_code: exec_output.exit_code,
                            duration: exec_output.duration,
                            formatted_output: format_exec_output_str(&exec_output),
                        }),
                    )
                    .await;
                let output_payload = format_exec_output_for_model(&exec_output);
                let output_text =
                    format!("<user_shell_command_output>\n{output_payload}\n</user_shell_command_output>");
                let output_items = [build_user_message(output_text)];
                session
                    .record_conversation_items(turn_context.as_ref(), &output_items)
                    .await;
            }
        }
        None
    }
}
