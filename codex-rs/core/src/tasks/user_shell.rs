use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use codex_async_utils::CancelErr;
use codex_async_utils::OrCancelExt;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;
use tracing::error;
use uuid::Uuid;

use crate::codex::TurnContext;
use crate::exec::ExecToolCallOutput;
use crate::exec::SandboxType;
use crate::exec::StdoutStream;
use crate::exec::StreamOutput;
use crate::exec::execute_exec_env;
use crate::exec_env::create_env;
use crate::parse_command::parse_command;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandBeginEvent;
use crate::protocol::ExecCommandEndEvent;
use crate::protocol::ExecCommandSource;
use crate::protocol::ExecCommandStatus;
use crate::protocol::SandboxPolicy;
use crate::protocol::TurnStartedEvent;
use crate::sandboxing::ExecRequest;
use crate::sandboxing::SandboxPermissions;
use crate::state::TaskKind;
use crate::tools::format_exec_output_str;
use crate::tools::runtimes::maybe_wrap_shell_lc_with_snapshot;
use crate::user_shell_command::user_shell_command_record_item;

use super::SessionTask;
use super::SessionTaskContext;
use crate::codex::Session;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;

const USER_SHELL_TIMEOUT_MS: u64 = 60 * 60 * 1000; // 1 hour

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UserShellCommandMode {
    /// Executes as an independent turn lifecycle (emits TurnStarted/TurnComplete
    /// via task lifecycle plumbing).
    StandaloneTurn,
    /// Executes while another turn is already active. This mode must not emit a
    /// second TurnStarted/TurnComplete pair for the same active turn.
    ActiveTurnAuxiliary,
}

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
        execute_user_shell_command(
            session.clone_session(),
            turn_context,
            self.command.clone(),
            cancellation_token,
            UserShellCommandMode::StandaloneTurn,
        )
        .await;
        None
    }
}

pub(crate) async fn execute_user_shell_command(
    session: Arc<Session>,
    turn_context: Arc<TurnContext>,
    command: String,
    cancellation_token: CancellationToken,
    mode: UserShellCommandMode,
) {
    session
        .services
        .otel_manager
        .counter("codex.task.user_shell", 1, &[]);

    if mode == UserShellCommandMode::StandaloneTurn {
        // Auxiliary mode runs within an existing active turn. That turn already
        // emitted TurnStarted, so emitting another TurnStarted here would create
        // duplicate turn lifecycle events and confuse clients.
        let event = EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: turn_context.sub_id.clone(),
            model_context_window: turn_context.model_context_window(),
            collaboration_mode_kind: turn_context.collaboration_mode.mode,
        });
        session.send_event(turn_context.as_ref(), event).await;
    }

    // Execute the user's script under their default shell when known; this
    // allows commands that use shell features (pipes, &&, redirects, etc.).
    // We do not source rc files or otherwise reformat the script.
    let use_login_shell = true;
    let session_shell = session.user_shell();
    let display_command = session_shell.derive_exec_args(&command, use_login_shell);
    let exec_command = maybe_wrap_shell_lc_with_snapshot(
        &display_command,
        session_shell.as_ref(),
        turn_context.cwd.as_path(),
        &turn_context.shell_environment_policy.r#set,
    );

    let call_id = Uuid::new_v4().to_string();
    let raw_command = command;
    let cwd = turn_context.cwd.clone();

    let parsed_cmd = parse_command(&display_command);
    session
        .send_event(
            turn_context.as_ref(),
            EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                call_id: call_id.clone(),
                process_id: None,
                turn_id: turn_context.sub_id.clone(),
                command: display_command.clone(),
                cwd: cwd.clone(),
                parsed_cmd: parsed_cmd.clone(),
                source: ExecCommandSource::UserShell,
                interaction_input: None,
            }),
        )
        .await;

    let exec_env = ExecRequest {
        command: exec_command.clone(),
        cwd: cwd.clone(),
        env: create_env(
            &turn_context.shell_environment_policy,
            Some(session.conversation_id),
        ),
        network: turn_context.network.clone(),
        // TODO(zhao-oai): Now that we have ExecExpiration::Cancellation, we
        // should use that instead of an "arbitrarily large" timeout here.
        expiration: USER_SHELL_TIMEOUT_MS.into(),
        sandbox: SandboxType::None,
        windows_sandbox_level: turn_context.windows_sandbox_level,
        sandbox_permissions: SandboxPermissions::UseDefault,
        justification: None,
        arg0: None,
    };

    let stdout_stream = Some(StdoutStream {
        sub_id: turn_context.sub_id.clone(),
        call_id: call_id.clone(),
        tx_event: session.get_tx_event(),
    });

    let sandbox_policy = SandboxPolicy::DangerFullAccess;
    let exec_result = execute_exec_env(exec_env, &sandbox_policy, stdout_stream)
        .or_cancel(&cancellation_token)
        .await;

    match exec_result {
        Err(CancelErr::Cancelled) => {
            let aborted_message = "command aborted by user".to_string();
            let exec_output = ExecToolCallOutput {
                exit_code: -1,
                stdout: StreamOutput::new(String::new()),
                stderr: StreamOutput::new(aborted_message.clone()),
                aggregated_output: StreamOutput::new(aborted_message.clone()),
                duration: Duration::ZERO,
                timed_out: false,
            };
            persist_user_shell_output(
                &session,
                turn_context.as_ref(),
                &raw_command,
                &exec_output,
                mode,
            )
            .await;
            session
                .send_event(
                    turn_context.as_ref(),
                    EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                        call_id,
                        process_id: None,
                        turn_id: turn_context.sub_id.clone(),
                        command: display_command.clone(),
                        cwd: cwd.clone(),
                        parsed_cmd: parsed_cmd.clone(),
                        source: ExecCommandSource::UserShell,
                        interaction_input: None,
                        stdout: String::new(),
                        stderr: aborted_message.clone(),
                        aggregated_output: aborted_message.clone(),
                        exit_code: -1,
                        duration: Duration::ZERO,
                        formatted_output: aborted_message,
                        status: ExecCommandStatus::Failed,
                    }),
                )
                .await;
        }
        Ok(Ok(output)) => {
            session
                .send_event(
                    turn_context.as_ref(),
                    EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                        call_id: call_id.clone(),
                        process_id: None,
                        turn_id: turn_context.sub_id.clone(),
                        command: display_command.clone(),
                        cwd: cwd.clone(),
                        parsed_cmd: parsed_cmd.clone(),
                        source: ExecCommandSource::UserShell,
                        interaction_input: None,
                        stdout: output.stdout.text.clone(),
                        stderr: output.stderr.text.clone(),
                        aggregated_output: output.aggregated_output.text.clone(),
                        exit_code: output.exit_code,
                        duration: output.duration,
                        formatted_output: format_exec_output_str(
                            &output,
                            turn_context.truncation_policy,
                        ),
                        status: if output.exit_code == 0 {
                            ExecCommandStatus::Completed
                        } else {
                            ExecCommandStatus::Failed
                        },
                    }),
                )
                .await;

            persist_user_shell_output(&session, turn_context.as_ref(), &raw_command, &output, mode)
                .await;
        }
        Ok(Err(err)) => {
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
                    EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                        call_id,
                        process_id: None,
                        turn_id: turn_context.sub_id.clone(),
                        command: display_command,
                        cwd,
                        parsed_cmd,
                        source: ExecCommandSource::UserShell,
                        interaction_input: None,
                        stdout: exec_output.stdout.text.clone(),
                        stderr: exec_output.stderr.text.clone(),
                        aggregated_output: exec_output.aggregated_output.text.clone(),
                        exit_code: exec_output.exit_code,
                        duration: exec_output.duration,
                        formatted_output: format_exec_output_str(
                            &exec_output,
                            turn_context.truncation_policy,
                        ),
                        status: ExecCommandStatus::Failed,
                    }),
                )
                .await;
            persist_user_shell_output(
                &session,
                turn_context.as_ref(),
                &raw_command,
                &exec_output,
                mode,
            )
            .await;
        }
    }
}

async fn persist_user_shell_output(
    session: &Session,
    turn_context: &TurnContext,
    raw_command: &str,
    exec_output: &ExecToolCallOutput,
    mode: UserShellCommandMode,
) {
    let output_item = user_shell_command_record_item(raw_command, exec_output, turn_context);

    if mode == UserShellCommandMode::StandaloneTurn {
        session
            .record_conversation_items(turn_context, std::slice::from_ref(&output_item))
            .await;
        return;
    }

    let response_input_item = match output_item {
        ResponseItem::Message { role, content, .. } => ResponseInputItem::Message { role, content },
        _ => unreachable!("user shell command output record should always be a message"),
    };

    if let Err(items) = session
        .inject_response_items(vec![response_input_item])
        .await
    {
        let response_items = items
            .into_iter()
            .map(ResponseItem::from)
            .collect::<Vec<_>>();
        session
            .record_conversation_items(turn_context, &response_items)
            .await;
    }
}
