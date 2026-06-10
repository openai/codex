use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use codex_async_utils::CancelErr;
use codex_async_utils::OrCancelExt;
use codex_exec_server::ExecEnvPolicy as ExecServerEnvPolicy;
use codex_exec_server::ExecOutputStream as ExecServerOutputStream;
use codex_exec_server::ExecParams as ExecServerParams;
use codex_network_proxy::PROXY_ACTIVE_ENV_KEY;
use codex_protocol::config_types::ShellEnvironmentPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio_util::sync::CancellationToken;
use tracing::error;
use uuid::Uuid;

use crate::exec::ExecCapturePolicy;
use crate::exec::StdoutStream;
use crate::exec::execute_exec_request;
use crate::exec_env::create_env;
use crate::sandboxing::ExecRequest;
use crate::session::TurnInput;
use crate::session::turn_context::TurnContext;
use crate::shell::Shell;
use crate::state::TaskKind;
use crate::tools::format_exec_output_str;
use crate::tools::runtimes::RuntimePathPrepends;
#[cfg(unix)]
use crate::tools::runtimes::apply_package_path_prepend;
use crate::tools::runtimes::maybe_wrap_shell_lc_with_snapshot;
use crate::tools::runtimes::strip_managed_proxy_env;
use crate::turn_timing::now_unix_timestamp_ms;
use crate::user_shell_command::user_shell_command_record_item;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::exec_output::StreamOutput;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::ExecCommandOutputDeltaEvent;
use codex_protocol::protocol::ExecCommandSource;
use codex_protocol::protocol::ExecCommandStatus;
use codex_protocol::protocol::ExecOutputStream as ProtocolExecOutputStream;
use codex_protocol::protocol::TurnStartedEvent;
use codex_sandboxing::SandboxType;
use codex_shell_command::parse_command::parse_command;

use super::SessionTask;
use super::SessionTaskContext;
use crate::session::session::Session;
use crate::session::turn_context::TurnEnvironment;
use codex_protocol::models::PermissionProfile;

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
    environment_id: Option<String>,
    command: String,
}

impl UserShellCommandTask {
    pub(crate) fn new(environment_id: Option<String>, command: String) -> Self {
        Self {
            environment_id,
            command,
        }
    }
}

impl SessionTask for UserShellCommandTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    fn span_name(&self) -> &'static str {
        "session_task.user_shell"
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        turn_context: Arc<TurnContext>,
        _input: Vec<TurnInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        execute_user_shell_command(
            session.clone_session(),
            turn_context,
            self.environment_id.clone(),
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
    environment_id: Option<String>,
    command: String,
    cancellation_token: CancellationToken,
    mode: UserShellCommandMode,
) {
    session
        .services
        .session_telemetry
        .counter("codex.task.user_shell", /*inc*/ 1, &[]);

    if mode == UserShellCommandMode::StandaloneTurn {
        // Auxiliary mode runs within an existing active turn. That turn already
        // emitted TurnStarted, so emitting another TurnStarted here would create
        // duplicate turn lifecycle events and confuse clients.
        // TODO(ccunningham): After TurnStarted, emit model-visible turn context diffs for
        // standalone lifecycle tasks (for example /shell, and review once it emits TurnStarted).
        // `/compact` is an intentional exception because compaction requests should not include
        // freshly reinjected context before the summary/replacement history is applied.
        let event = EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: turn_context.sub_id.clone(),
            trace_id: turn_context.trace_id.clone(),
            started_at: turn_context.turn_timing_state.started_at_unix_secs().await,
            model_context_window: turn_context.model_context_window(),
            collaboration_mode_kind: turn_context.collaboration_mode.mode,
        });
        session.send_event(turn_context.as_ref(), event).await;
    }

    if let Some(environment_id) = environment_id {
        execute_remote_user_shell_command(
            session,
            turn_context,
            environment_id,
            command,
            cancellation_token,
            mode,
        )
        .await;
        return;
    }

    // Execute the user's script under their default shell when known; this
    // allows commands that use shell features (pipes, &&, redirects, etc.).
    // We do not source rc files or otherwise reformat the script.
    let use_login_shell = true;
    let session_shell = session.user_shell();
    let display_command = session_shell.derive_exec_args(&command, use_login_shell);
    let mut exec_env_map = create_env(
        &turn_context.shell_environment_policy,
        Some(session.thread_id),
    );
    if exec_env_map.contains_key(PROXY_ACTIVE_ENV_KEY) {
        strip_managed_proxy_env(&mut exec_env_map);
    }
    let exec_command = prepare_user_shell_exec_command(
        &display_command,
        session_shell.as_ref(),
        #[allow(deprecated)]
        &turn_context.cwd,
        &turn_context.shell_environment_policy.r#set,
        &mut exec_env_map,
    );

    let call_id = Uuid::new_v4().to_string();
    let raw_command = command;
    #[allow(deprecated)]
    let cwd = turn_context.cwd.clone();

    let parsed_cmd = parse_command(&display_command);
    session
        .send_event(
            turn_context.as_ref(),
            EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                call_id: call_id.clone(),
                process_id: None,
                turn_id: turn_context.sub_id.clone(),
                started_at_ms: now_unix_timestamp_ms(),
                command: display_command.clone(),
                cwd: cwd.clone(),
                parsed_cmd: parsed_cmd.clone(),
                source: ExecCommandSource::UserShell,
                interaction_input: None,
            }),
        )
        .await;

    let permission_profile = PermissionProfile::Disabled;
    let exec_env = ExecRequest {
        command: exec_command.clone(),
        cwd: cwd.clone(),
        env: exec_env_map,
        exec_server_env_config: None,
        // `/shell` is the explicit full-access escape hatch, so it must not
        // inherit a managed proxy from the surrounding session or turn.
        network: None,
        // TODO(zhao-oai): Now that we have ExecExpiration::Cancellation, we
        // should use that instead of an "arbitrarily large" timeout here.
        expiration: USER_SHELL_TIMEOUT_MS.into(),
        capture_policy: ExecCapturePolicy::ShellTool,
        sandbox: SandboxType::None,
        windows_sandbox_policy_cwd: cwd.clone(),
        windows_sandbox_workspace_roots: turn_context.config.effective_workspace_roots(),
        windows_sandbox_level: turn_context.windows_sandbox_level,
        windows_sandbox_private_desktop: turn_context
            .config
            .permissions
            .windows_sandbox_private_desktop,
        permission_profile: permission_profile.clone(),
        file_system_sandbox_policy: permission_profile.file_system_sandbox_policy(),
        network_sandbox_policy: permission_profile.network_sandbox_policy(),
        windows_sandbox_filesystem_overrides: None,
        arg0: None,
    };

    let stdout_stream = Some(StdoutStream {
        sub_id: turn_context.sub_id.clone(),
        call_id: call_id.clone(),
        tx_event: session.get_tx_event(),
    });

    let exec_result = execute_exec_request(exec_env, stdout_stream, /*after_spawn*/ None)
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
                        completed_at_ms: now_unix_timestamp_ms(),
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
                        completed_at_ms: now_unix_timestamp_ms(),
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
                        completed_at_ms: now_unix_timestamp_ms(),
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

async fn execute_remote_user_shell_command(
    session: Arc<Session>,
    turn_context: Arc<TurnContext>,
    environment_id: String,
    command: String,
    cancellation_token: CancellationToken,
    mode: UserShellCommandMode,
) {
    let Some(turn_environment) = turn_context
        .environments
        .turn_environments
        .iter()
        .find(|environment| environment.environment_id == environment_id)
        .cloned()
    else {
        let message = format!("unknown turn environment id `{environment_id}`");
        emit_user_shell_failed_without_begin(&session, turn_context.as_ref(), &command, &message)
            .await;
        return;
    };

    if !turn_environment.environment.is_remote() {
        let message = format!("user shell environment `{environment_id}` is not remote");
        emit_user_shell_failed_without_begin(&session, turn_context.as_ref(), &command, &message)
            .await;
        return;
    }

    let remote_shell = match remote_user_shell(&turn_environment).await {
        Ok(shell) => shell,
        Err(message) => {
            emit_user_shell_failed_without_begin(
                &session,
                turn_context.as_ref(),
                &command,
                &message,
            )
            .await;
            return;
        }
    };
    let use_login_shell = true;
    let display_command = remote_shell.derive_exec_args(&command, use_login_shell);
    let parsed_cmd = parse_command(&display_command);
    let call_id = Uuid::new_v4().to_string();
    let raw_command = command;
    let cwd = turn_environment.cwd.clone();

    session
        .send_event(
            turn_context.as_ref(),
            EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                call_id: call_id.clone(),
                process_id: None,
                turn_id: turn_context.sub_id.clone(),
                started_at_ms: now_unix_timestamp_ms(),
                command: display_command.clone(),
                cwd: cwd.clone(),
                parsed_cmd: parsed_cmd.clone(),
                source: ExecCommandSource::UserShell,
                interaction_input: None,
            }),
        )
        .await;

    let process_id = Uuid::new_v4().to_string();
    let exec_params = ExecServerParams {
        process_id: codex_exec_server::ProcessId::new(process_id),
        argv: display_command.clone(),
        cwd: cwd.as_path().to_path_buf(),
        env_policy: Some(exec_server_env_policy(
            &turn_context.shell_environment_policy,
        )),
        env: HashMap::new(),
        tty: false,
        pipe_stdin: false,
        arg0: None,
    };
    let start = Instant::now();
    let exec_result = collect_remote_user_shell_output(
        turn_environment.environment.get_exec_backend(),
        exec_params,
        StdoutStream {
            sub_id: turn_context.sub_id.clone(),
            call_id: call_id.clone(),
            tx_event: session.get_tx_event(),
        },
        cancellation_token,
    )
    .await;

    let (exec_output, status) = match exec_result {
        RemoteUserShellResult::Output {
            exit_code,
            stdout,
            stderr,
            aggregated_output,
        } => {
            let output = ExecToolCallOutput {
                exit_code,
                stdout: StreamOutput::new(String::from_utf8_lossy(&stdout).into_owned()),
                stderr: StreamOutput::new(String::from_utf8_lossy(&stderr).into_owned()),
                aggregated_output: StreamOutput::new(
                    String::from_utf8_lossy(&aggregated_output).into_owned(),
                ),
                duration: start.elapsed(),
                timed_out: false,
            };
            let status = if exit_code == 0 {
                ExecCommandStatus::Completed
            } else {
                ExecCommandStatus::Failed
            };
            (output, status)
        }
        RemoteUserShellResult::Cancelled => {
            let aborted_message = "command aborted by user".to_string();
            (
                ExecToolCallOutput {
                    exit_code: -1,
                    stdout: StreamOutput::new(String::new()),
                    stderr: StreamOutput::new(aborted_message.clone()),
                    aggregated_output: StreamOutput::new(aborted_message),
                    duration: start.elapsed(),
                    timed_out: false,
                },
                ExecCommandStatus::Failed,
            )
        }
        RemoteUserShellResult::Error(message) => (
            ExecToolCallOutput {
                exit_code: -1,
                stdout: StreamOutput::new(String::new()),
                stderr: StreamOutput::new(message.clone()),
                aggregated_output: StreamOutput::new(message),
                duration: start.elapsed(),
                timed_out: false,
            },
            ExecCommandStatus::Failed,
        ),
    };

    session
        .send_event(
            turn_context.as_ref(),
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id,
                process_id: None,
                turn_id: turn_context.sub_id.clone(),
                completed_at_ms: now_unix_timestamp_ms(),
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
                status,
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

async fn emit_user_shell_failed_without_begin(
    session: &Session,
    turn_context: &TurnContext,
    raw_command: &str,
    message: &str,
) {
    let exec_output = ExecToolCallOutput {
        exit_code: -1,
        stdout: StreamOutput::new(String::new()),
        stderr: StreamOutput::new(message.to_string()),
        aggregated_output: StreamOutput::new(message.to_string()),
        duration: Duration::ZERO,
        timed_out: false,
    };
    persist_user_shell_output(
        session,
        turn_context,
        raw_command,
        &exec_output,
        UserShellCommandMode::StandaloneTurn,
    )
    .await;
}

async fn remote_user_shell(turn_environment: &TurnEnvironment) -> Result<Shell, String> {
    let info = turn_environment
        .environment
        .info()
        .await
        .map_err(|err| format!("failed to read environment shell info: {err}"))?;
    let shell_type = match info.shell.name.as_str() {
        "zsh" => crate::shell::ShellType::Zsh,
        "bash" => crate::shell::ShellType::Bash,
        "powershell" => crate::shell::ShellType::PowerShell,
        "sh" => crate::shell::ShellType::Sh,
        "cmd" => crate::shell::ShellType::Cmd,
        name => return Err(format!("unsupported remote shell `{name}`")),
    };
    Ok(Shell {
        shell_type,
        shell_path: info.shell.path.into(),
        shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
    })
}

fn exec_server_env_policy(policy: &ShellEnvironmentPolicy) -> ExecServerEnvPolicy {
    ExecServerEnvPolicy {
        inherit: policy.inherit.clone(),
        ignore_default_excludes: policy.ignore_default_excludes,
        exclude: policy.exclude.iter().map(ToString::to_string).collect(),
        r#set: policy.r#set.clone(),
        include_only: policy
            .include_only
            .iter()
            .map(ToString::to_string)
            .collect(),
    }
}

enum RemoteUserShellResult {
    Output {
        exit_code: i32,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        aggregated_output: Vec<u8>,
    },
    Cancelled,
    Error(String),
}

async fn collect_remote_user_shell_output(
    exec_backend: Arc<dyn codex_exec_server::ExecBackend>,
    exec_params: ExecServerParams,
    stream: StdoutStream,
    cancellation_token: CancellationToken,
) -> RemoteUserShellResult {
    let process = match exec_backend.start(exec_params).await {
        Ok(started) => started.process,
        Err(err) => return RemoteUserShellResult::Error(format!("execution error: {err}")),
    };
    let mut after_seq = None;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut aggregated_output = Vec::new();
    let mut exit_code = None;

    loop {
        let read = tokio::select! {
            _ = cancellation_token.cancelled() => {
                let _ = process.terminate().await;
                return RemoteUserShellResult::Cancelled;
            }
            read = process.read(after_seq, None, Some(100)) => read,
        };
        let read = match read {
            Ok(read) => read,
            Err(err) => return RemoteUserShellResult::Error(format!("execution error: {err}")),
        };
        after_seq = Some(read.next_seq);
        if let Some(code) = read.exit_code {
            exit_code = Some(code);
        }
        if let Some(failure) = read.failure {
            return RemoteUserShellResult::Error(failure);
        }
        for chunk in read.chunks {
            let (protocol_stream, target) = match chunk.stream {
                ExecServerOutputStream::Stdout | ExecServerOutputStream::Pty => {
                    (ProtocolExecOutputStream::Stdout, &mut stdout)
                }
                ExecServerOutputStream::Stderr => (ProtocolExecOutputStream::Stderr, &mut stderr),
            };
            let bytes = chunk.chunk.into_inner();
            aggregated_output.extend_from_slice(&bytes);
            target.extend_from_slice(&bytes);
            let _ = stream
                .tx_event
                .send(Event {
                    id: stream.sub_id.clone(),
                    msg: EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                        call_id: stream.call_id.clone(),
                        stream: protocol_stream,
                        chunk: bytes,
                    }),
                })
                .await;
        }
        if read.closed {
            return RemoteUserShellResult::Output {
                exit_code: exit_code.unwrap_or(-1),
                stdout,
                stderr,
                aggregated_output,
            };
        }
    }
}

fn prepare_user_shell_exec_command(
    display_command: &[String],
    session_shell: &Shell,
    cwd: &AbsolutePathBuf,
    shell_environment_set: &HashMap<String, String>,
    exec_env_map: &mut HashMap<String, String>,
) -> Vec<String> {
    #[cfg(unix)]
    {
        prepare_user_shell_exec_command_with_path_prepend(
            display_command,
            session_shell,
            cwd,
            shell_environment_set,
            exec_env_map,
            apply_package_path_prepend,
        )
    }

    #[cfg(not(unix))]
    {
        maybe_wrap_shell_lc_with_snapshot(
            display_command,
            session_shell,
            cwd,
            shell_environment_set,
            exec_env_map,
            // On non-Unix targets, arg0 has already prepended the package path
            // to the process PATH before create_env() builds exec_env_map.
            // RuntimePathPrepends is only needed for Unix shell snapshot replay.
            &RuntimePathPrepends::default(),
        )
    }
}

/// Prepares a user-shell command after adding runtime-owned PATH entries.
///
/// The callback mutates the live exec environment for commands that are not
/// wrapped with a shell snapshot and records only the runtime-owned entries so
/// snapshot wrapping can reapply them after restoring the user's snapshot PATH.
#[cfg(unix)]
fn prepare_user_shell_exec_command_with_path_prepend(
    display_command: &[String],
    session_shell: &Shell,
    cwd: &AbsolutePathBuf,
    shell_environment_set: &HashMap<String, String>,
    exec_env_map: &mut HashMap<String, String>,
    prepend_runtime_path: impl FnOnce(&mut HashMap<String, String>, &mut RuntimePathPrepends),
) -> Vec<String> {
    let explicit_env_overrides = shell_environment_set.clone();
    let mut runtime_path_prepends = RuntimePathPrepends::default();
    prepend_runtime_path(exec_env_map, &mut runtime_path_prepends);
    maybe_wrap_shell_lc_with_snapshot(
        display_command,
        session_shell,
        cwd,
        &explicit_env_overrides,
        exec_env_map,
        &runtime_path_prepends,
    )
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
        // Standalone shell turns can run before any regular user turn, so
        // explicitly materialize rollout persistence after recording output.
        session.ensure_rollout_materialized().await;
        return;
    }

    session
        .inject_no_new_turn(vec![output_item], Some(turn_context))
        .await;
}

#[cfg(all(test, unix))]
#[path = "user_shell_tests.rs"]
mod tests;
