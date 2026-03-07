use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_protocol::CommandExecOutputDeltaNotification;
use codex_app_server_protocol::CommandExecOutputStream;
use codex_app_server_protocol::CommandExecResizeParams;
use codex_app_server_protocol::CommandExecResizeResponse;
use codex_app_server_protocol::CommandExecResponse;
use codex_app_server_protocol::CommandExecTerminalSize;
use codex_app_server_protocol::CommandExecTerminateParams;
use codex_app_server_protocol::CommandExecTerminateResponse;
use codex_app_server_protocol::CommandExecWriteParams;
use codex_app_server_protocol::CommandExecWriteResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ServerNotification;
use codex_core::bytes_to_string_smart;
use codex_core::config::StartedNetworkProxy;
use codex_core::exec::DEFAULT_EXEC_COMMAND_TIMEOUT_MS;
use codex_core::exec::ExecExpiration;
use codex_core::exec::IO_DRAIN_TIMEOUT_MS;
use codex_core::exec::SandboxType;
use codex_core::sandboxing::ExecRequest;
use codex_utils_pty::DEFAULT_OUTPUT_BYTES_CAP;
use codex_utils_pty::ProcessHandle;
use codex_utils_pty::SpawnedProcess;
use codex_utils_pty::TerminalSize;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::time::Instant;

use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_PARAMS_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::ConnectionRequestId;
use crate::outgoing_message::OutgoingMessageSender;

const EXEC_TIMEOUT_EXIT_CODE: i32 = 124;
const POST_EXIT_QUIET_PERIOD_MS: u64 = 50;
const POST_EXIT_READY_DRAIN_LIMIT: usize = 256;

#[derive(Clone)]
pub(crate) struct CommandExecManager {
    sessions: Arc<Mutex<HashMap<ConnectionProcessId, CommandExecSession>>>,
    next_generated_process_id: Arc<AtomicI64>,
}

impl Default for CommandExecManager {
    fn default() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            next_generated_process_id: Arc::new(AtomicI64::new(1)),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ConnectionProcessId {
    connection_id: ConnectionId,
    process_id: InternalProcessId,
}

#[derive(Clone)]
enum CommandExecSession {
    Active {
        control_tx: mpsc::Sender<CommandControlRequest>,
    },
    UnsupportedWindowsSandbox,
}

enum CommandControl {
    Write { delta: Vec<u8>, close_stdin: bool },
    Resize { size: TerminalSize },
    Terminate,
}

struct CommandControlRequest {
    control: CommandControl,
    response_tx: Option<oneshot::Sender<Result<(), JSONRPCErrorError>>>,
}

pub(crate) struct StartCommandExecParams {
    pub(crate) outgoing: Arc<OutgoingMessageSender>,
    pub(crate) request_id: ConnectionRequestId,
    pub(crate) process_id: Option<String>,
    pub(crate) exec_request: ExecRequest,
    pub(crate) started_network_proxy: Option<StartedNetworkProxy>,
    pub(crate) tty: bool,
    pub(crate) stream_stdin: bool,
    pub(crate) stream_stdout_stderr: bool,
    pub(crate) output_bytes_cap: Option<usize>,
    pub(crate) size: Option<TerminalSize>,
}

struct RunCommandParams {
    outgoing: Arc<OutgoingMessageSender>,
    request_id: ConnectionRequestId,
    process_id: Option<String>,
    spawned: SpawnedProcess,
    control_rx: mpsc::Receiver<CommandControlRequest>,
    stream_stdin: bool,
    stream_stdout_stderr: bool,
    expiration: ExecExpiration,
    output_bytes_cap: Option<usize>,
}

struct SpawnProcessOutputParams {
    connection_id: ConnectionId,
    process_id: Option<String>,
    output_rx: mpsc::Receiver<Vec<u8>>,
    process_exited_rx: watch::Receiver<bool>,
    outgoing: Arc<OutgoingMessageSender>,
    stream: CommandExecOutputStream,
    stream_output: bool,
    output_bytes_cap: Option<usize>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum InternalProcessId {
    Generated(i64),
    Client(String),
}

trait InternalProcessIdExt {
    fn error_repr(&self) -> String;
}

impl InternalProcessIdExt for InternalProcessId {
    fn error_repr(&self) -> String {
        match self {
            Self::Generated(id) => id.to_string(),
            Self::Client(id) => serde_json::to_string(id).unwrap_or_else(|_| format!("{id:?}")),
        }
    }
}

impl CommandExecManager {
    pub(crate) async fn start(
        &self,
        params: StartCommandExecParams,
    ) -> Result<(), JSONRPCErrorError> {
        let StartCommandExecParams {
            outgoing,
            request_id,
            process_id,
            exec_request,
            started_network_proxy,
            tty,
            stream_stdin,
            stream_stdout_stderr,
            output_bytes_cap,
            size,
        } = params;
        if process_id.is_none() && (tty || stream_stdin || stream_stdout_stderr) {
            return Err(invalid_request(
                "command/exec tty or streaming requires a client-supplied processId".to_string(),
            ));
        }
        let process_id = process_id.map_or_else(
            || {
                InternalProcessId::Generated(
                    self.next_generated_process_id
                        .fetch_add(1, Ordering::Relaxed),
                )
            },
            InternalProcessId::Client,
        );
        let process_key = ConnectionProcessId {
            connection_id: request_id.connection_id,
            process_id: process_id.clone(),
        };

        if matches!(exec_request.sandbox, SandboxType::WindowsRestrictedToken) {
            if matches!(&exec_request.expiration, ExecExpiration::Cancellation(_)) {
                return Err(invalid_request(
                    "disableTimeout is not supported with windows sandbox".to_string(),
                ));
            }
            if tty || stream_stdin || stream_stdout_stderr {
                return Err(invalid_request(
                    "streaming command/exec is not supported with windows sandbox".to_string(),
                ));
            }
            if output_bytes_cap != Some(DEFAULT_OUTPUT_BYTES_CAP) {
                return Err(invalid_request(
                    "custom outputBytesCap is not supported with windows sandbox".to_string(),
                ));
            }
            if let InternalProcessId::Client(_) = &process_id {
                let mut sessions = self.sessions.lock().await;
                if sessions.contains_key(&process_key) {
                    return Err(invalid_request(format!(
                        "duplicate active command/exec process id: {}",
                        process_key.process_id.error_repr(),
                    )));
                }
                sessions.insert(
                    process_key.clone(),
                    CommandExecSession::UnsupportedWindowsSandbox,
                );
            }
            let sessions = Arc::clone(&self.sessions);
            tokio::spawn(async move {
                let _started_network_proxy = started_network_proxy;
                match codex_core::sandboxing::execute_env(exec_request, None).await {
                    Ok(output) => {
                        outgoing
                            .send_response(
                                request_id,
                                CommandExecResponse {
                                    exit_code: output.exit_code,
                                    stdout: output.stdout.text,
                                    stderr: output.stderr.text,
                                },
                            )
                            .await;
                    }
                    Err(err) => {
                        outgoing
                            .send_error(request_id, internal_error(format!("exec failed: {err}")))
                            .await;
                    }
                }
                sessions.lock().await.remove(&process_key);
            });
            return Ok(());
        }

        let ExecRequest {
            command,
            cwd,
            env,
            expiration,
            sandbox: _sandbox,
            arg0,
            ..
        } = exec_request;

        let stream_stdin = tty || stream_stdin;
        let stream_stdout_stderr = tty || stream_stdout_stderr;
        let (control_tx, control_rx) = mpsc::channel(32);
        let notification_process_id = match &process_id {
            InternalProcessId::Generated(_) => None,
            InternalProcessId::Client(process_id) => Some(process_id.clone()),
        };

        let sessions = Arc::clone(&self.sessions);
        let (program, args) = command
            .split_first()
            .ok_or_else(|| invalid_request("command must not be empty".to_string()))?;
        {
            let mut sessions = self.sessions.lock().await;
            if sessions.contains_key(&process_key) {
                return Err(invalid_request(format!(
                    "duplicate active command/exec process id: {}",
                    process_key.process_id.error_repr(),
                )));
            }
            sessions.insert(
                process_key.clone(),
                CommandExecSession::Active { control_tx },
            );
        }
        let spawned = if tty {
            codex_utils_pty::spawn_pty_process(
                program,
                args,
                cwd.as_path(),
                &env,
                &arg0,
                size.unwrap_or_default(),
            )
            .await
        } else if stream_stdin {
            codex_utils_pty::spawn_pipe_process(program, args, cwd.as_path(), &env, &arg0).await
        } else {
            codex_utils_pty::spawn_pipe_process_no_stdin(program, args, cwd.as_path(), &env, &arg0)
                .await
        };
        let spawned = match spawned {
            Ok(spawned) => spawned,
            Err(err) => {
                self.sessions.lock().await.remove(&process_key);
                return Err(internal_error(format!("failed to spawn command: {err}")));
            }
        };
        tokio::spawn(async move {
            let _started_network_proxy = started_network_proxy;
            run_command(RunCommandParams {
                outgoing,
                request_id: request_id.clone(),
                process_id: notification_process_id,
                spawned,
                control_rx,
                stream_stdin,
                stream_stdout_stderr,
                expiration,
                output_bytes_cap,
            })
            .await;
            sessions.lock().await.remove(&process_key);
        });
        Ok(())
    }

    pub(crate) async fn write(
        &self,
        request_id: ConnectionRequestId,
        params: CommandExecWriteParams,
    ) -> Result<CommandExecWriteResponse, JSONRPCErrorError> {
        if params.delta_base64.is_none() && !params.close_stdin {
            return Err(invalid_params(
                "command/exec/write requires deltaBase64 or closeStdin".to_string(),
            ));
        }

        let delta = match params.delta_base64 {
            Some(delta_base64) => STANDARD
                .decode(delta_base64)
                .map_err(|err| invalid_params(format!("invalid deltaBase64: {err}")))?,
            None => Vec::new(),
        };

        let target_process_id = ConnectionProcessId {
            connection_id: request_id.connection_id,
            process_id: InternalProcessId::Client(params.process_id),
        };
        self.send_control(
            target_process_id,
            CommandControl::Write {
                delta,
                close_stdin: params.close_stdin,
            },
        )
        .await?;

        Ok(CommandExecWriteResponse {})
    }

    pub(crate) async fn terminate(
        &self,
        request_id: ConnectionRequestId,
        params: CommandExecTerminateParams,
    ) -> Result<CommandExecTerminateResponse, JSONRPCErrorError> {
        let target_process_id = ConnectionProcessId {
            connection_id: request_id.connection_id,
            process_id: InternalProcessId::Client(params.process_id),
        };
        self.send_control(target_process_id, CommandControl::Terminate)
            .await?;
        Ok(CommandExecTerminateResponse {})
    }

    pub(crate) async fn resize(
        &self,
        request_id: ConnectionRequestId,
        params: CommandExecResizeParams,
    ) -> Result<CommandExecResizeResponse, JSONRPCErrorError> {
        let target_process_id = ConnectionProcessId {
            connection_id: request_id.connection_id,
            process_id: InternalProcessId::Client(params.process_id),
        };
        self.send_control(
            target_process_id,
            CommandControl::Resize {
                size: terminal_size_from_protocol(params.size)?,
            },
        )
        .await?;
        Ok(CommandExecResizeResponse {})
    }

    pub(crate) async fn connection_closed(&self, connection_id: ConnectionId) {
        let controls = {
            let mut sessions = self.sessions.lock().await;
            let process_ids = sessions
                .keys()
                .filter(|process_id| process_id.connection_id == connection_id)
                .cloned()
                .collect::<Vec<_>>();
            let mut controls = Vec::with_capacity(process_ids.len());
            for process_id in process_ids {
                if let Some(control) = sessions.remove(&process_id) {
                    controls.push(control);
                }
            }
            controls
        };

        for control in controls {
            if let CommandExecSession::Active { control_tx } = control {
                let _ = control_tx
                    .send(CommandControlRequest {
                        control: CommandControl::Terminate,
                        response_tx: None,
                    })
                    .await;
            }
        }
    }

    async fn send_control(
        &self,
        process_id: ConnectionProcessId,
        control: CommandControl,
    ) -> Result<(), JSONRPCErrorError> {
        let session = {
            self.sessions
                .lock()
                .await
                .get(&process_id)
                .cloned()
                .ok_or_else(|| {
                    invalid_request(format!(
                        "no active command/exec for process id {}",
                        process_id.process_id.error_repr(),
                    ))
                })?
        };
        let CommandExecSession::Active { control_tx } = session else {
            return Err(invalid_request(
                "command/exec/write, command/exec/terminate, and command/exec/resize are not supported for windows sandbox processes".to_string(),
            ));
        };
        let (response_tx, response_rx) = oneshot::channel();
        let request = CommandControlRequest {
            control,
            response_tx: Some(response_tx),
        };
        control_tx
            .send(request)
            .await
            .map_err(|_| command_no_longer_running_error(&process_id.process_id))?;
        response_rx
            .await
            .map_err(|_| command_no_longer_running_error(&process_id.process_id))?
    }
}

async fn run_command(params: RunCommandParams) {
    let RunCommandParams {
        outgoing,
        request_id,
        process_id,
        spawned,
        control_rx,
        stream_stdin,
        stream_stdout_stderr,
        expiration,
        output_bytes_cap,
    } = params;
    let mut control_rx = control_rx;
    let mut control_open = true;
    let expiration = async {
        match expiration {
            ExecExpiration::Timeout(duration) => tokio::time::sleep(duration).await,
            ExecExpiration::DefaultTimeout => {
                tokio::time::sleep(Duration::from_millis(DEFAULT_EXEC_COMMAND_TIMEOUT_MS)).await;
            }
            ExecExpiration::Cancellation(cancel) => {
                cancel.cancelled().await;
            }
        }
    };
    tokio::pin!(expiration);
    let SpawnedProcess {
        session,
        stdout_rx,
        stderr_rx,
        exit_rx,
    } = spawned;
    tokio::pin!(exit_rx);
    let mut timed_out = false;
    let (process_exited_tx, process_exited_rx) = watch::channel(false);

    let stdout_handle = spawn_process_output(SpawnProcessOutputParams {
        connection_id: request_id.connection_id,
        process_id: process_id.clone(),
        output_rx: stdout_rx,
        process_exited_rx: process_exited_rx.clone(),
        outgoing: Arc::clone(&outgoing),
        stream: CommandExecOutputStream::Stdout,
        stream_output: stream_stdout_stderr,
        output_bytes_cap,
    });
    let stderr_handle = spawn_process_output(SpawnProcessOutputParams {
        connection_id: request_id.connection_id,
        process_id,
        output_rx: stderr_rx,
        process_exited_rx,
        outgoing: Arc::clone(&outgoing),
        stream: CommandExecOutputStream::Stderr,
        stream_output: stream_stdout_stderr,
        output_bytes_cap,
    });

    let exit_code = loop {
        tokio::select! {
            control = control_rx.recv(), if control_open => {
                match control {
                    Some(CommandControlRequest { control, response_tx }) => {
                        let result = match control {
                            CommandControl::Write { delta, close_stdin } => {
                                handle_process_write(
                                    &session,
                                    stream_stdin,
                                    delta,
                                    close_stdin,
                                ).await
                            }
                            CommandControl::Resize { size } => {
                                handle_process_resize(&session, size)
                            }
                            CommandControl::Terminate => {
                                session.request_terminate();
                                Ok(())
                            }
                        };
                        if let Some(response_tx) = response_tx {
                            let _ = response_tx.send(result);
                        }
                    },
                    None => {
                        control_open = false;
                        session.request_terminate();
                    }
                }
            }
            _ = &mut expiration, if !timed_out => {
                timed_out = true;
                session.request_terminate();
            }
            exit = &mut exit_rx => {
                if timed_out {
                    break EXEC_TIMEOUT_EXIT_CODE;
                } else {
                    break exit.unwrap_or(-1);
                }
            }
        }
    };

    // The child has exited, so future control RPCs should fail immediately
    // instead of waiting for the post-exit output drain to finish.
    drop(control_rx);

    let _ = process_exited_tx.send(true);
    let stdout = stdout_handle.await.unwrap_or_default();
    let stderr = stderr_handle.await.unwrap_or_default();

    outgoing
        .send_response(
            request_id,
            CommandExecResponse {
                exit_code,
                stdout,
                stderr,
            },
        )
        .await;
}

fn spawn_process_output(params: SpawnProcessOutputParams) -> tokio::task::JoinHandle<String> {
    tokio::spawn(async move {
        collect_process_output(
            params,
            Duration::from_millis(POST_EXIT_QUIET_PERIOD_MS),
            Duration::from_millis(IO_DRAIN_TIMEOUT_MS),
        )
        .await
    })
}

struct ProcessOutputChunkContext<'a> {
    outgoing: &'a OutgoingMessageSender,
    connection_id: ConnectionId,
    process_id: Option<&'a String>,
    stream: CommandExecOutputStream,
    stream_output: bool,
    output_bytes_cap: Option<usize>,
}

async fn collect_process_output(
    params: SpawnProcessOutputParams,
    post_exit_quiet_period: Duration,
    post_exit_hard_cap: Duration,
) -> String {
    let SpawnProcessOutputParams {
        connection_id,
        process_id,
        mut output_rx,
        mut process_exited_rx,
        outgoing,
        stream,
        stream_output,
        output_bytes_cap,
    } = params;

    let mut buffer = Vec::new();
    let mut observed_num_bytes = 0usize;
    let mut post_exit_deadlines = (*process_exited_rx.borrow()).then(|| {
        let now = Instant::now();
        (now + post_exit_quiet_period, now + post_exit_hard_cap)
    });
    let chunk_context = ProcessOutputChunkContext {
        outgoing: &outgoing,
        connection_id,
        process_id: process_id.as_ref(),
        stream,
        stream_output,
        output_bytes_cap,
    };

    loop {
        let mut drained_any = false;
        // Once the hard cap elapses, do one last bounded `try_recv` pass so already-ready
        // tail bytes win over the timeout path instead of being dropped by a select race.
        let mut ready_drain_limit = post_exit_deadlines
            .as_ref()
            .filter(|(_, hard_deadline)| Instant::now() >= *hard_deadline)
            .map(|_| POST_EXIT_READY_DRAIN_LIMIT);

        while !matches!(ready_drain_limit, Some(0)) {
            match output_rx.try_recv() {
                Ok(chunk) => {
                    drained_any = true;
                    if process_output_chunk(
                        &chunk_context,
                        &mut observed_num_bytes,
                        &mut buffer,
                        chunk,
                    )
                    .await
                    {
                        return bytes_to_string_smart(&buffer);
                    }
                    if let Some(limit) = ready_drain_limit.as_mut() {
                        *limit = limit.saturating_sub(1);
                    } else if post_exit_deadlines
                        .as_ref()
                        .is_some_and(|(_, hard_deadline)| Instant::now() >= *hard_deadline)
                    {
                        ready_drain_limit = Some(POST_EXIT_READY_DRAIN_LIMIT);
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return bytes_to_string_smart(&buffer),
            }
        }

        // Process exit starts the post-exit quiet window, but we still wait a bit longer for
        // trailing bytes until either the stream goes quiet or the hard cap expires.
        if post_exit_deadlines.is_none() && *process_exited_rx.borrow() {
            let now = Instant::now();
            post_exit_deadlines = Some((now + post_exit_quiet_period, now + post_exit_hard_cap));
        }

        if drained_any {
            if let Some((quiet_deadline, _)) = post_exit_deadlines.as_mut() {
                *quiet_deadline = Instant::now() + post_exit_quiet_period;
            }
            continue;
        }

        let next_deadline = post_exit_deadlines
            .as_ref()
            .map(|(quiet_deadline, hard_deadline)| (*quiet_deadline).min(*hard_deadline));
        if next_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            break;
        }
        tokio::select! {
            chunk = output_rx.recv() => match chunk {
                Some(chunk) => {
                    if process_output_chunk(
                        &chunk_context,
                        &mut observed_num_bytes,
                        &mut buffer,
                        chunk,
                    )
                    .await {
                        break;
                    }
                    if let Some((quiet_deadline, _)) = post_exit_deadlines.as_mut() {
                        *quiet_deadline = Instant::now() + post_exit_quiet_period;
                    }
                }
                None => break,
            },
            // Before exit we wait for the exit signal; after exit we swap that branch for the
            // next post-exit deadline so the same select handles both phases.
            _ = tokio::time::sleep_until(next_deadline.unwrap_or_else(Instant::now)),
                if next_deadline.is_some() => {}
            changed = process_exited_rx.changed(), if next_deadline.is_none() => {
                if changed.is_err() || *process_exited_rx.borrow() {
                    let now = Instant::now();
                    post_exit_deadlines = Some((now + post_exit_quiet_period, now + post_exit_hard_cap));
                }
            }
        }
    }

    bytes_to_string_smart(&buffer)
}

async fn process_output_chunk(
    context: &ProcessOutputChunkContext<'_>,
    observed_num_bytes: &mut usize,
    buffer: &mut Vec<u8>,
    chunk: Vec<u8>,
) -> bool {
    let capped_chunk = match context.output_bytes_cap {
        Some(output_bytes_cap) => {
            let capped_chunk_len = output_bytes_cap
                .saturating_sub(*observed_num_bytes)
                .min(chunk.len());
            *observed_num_bytes += capped_chunk_len;
            &chunk[0..capped_chunk_len]
        }
        None => chunk.as_slice(),
    };
    let cap_reached = Some(*observed_num_bytes) == context.output_bytes_cap;
    if let (true, Some(process_id)) = (context.stream_output, context.process_id) {
        context
            .outgoing
            .send_server_notification_to_connections(
                &[context.connection_id],
                ServerNotification::CommandExecOutputDelta(CommandExecOutputDeltaNotification {
                    process_id: process_id.clone(),
                    stream: context.stream,
                    delta_base64: STANDARD.encode(capped_chunk),
                    cap_reached,
                }),
            )
            .await;
    } else if !context.stream_output {
        buffer.extend_from_slice(capped_chunk);
    }
    cap_reached
}

async fn handle_process_write(
    session: &ProcessHandle,
    stream_stdin: bool,
    delta: Vec<u8>,
    close_stdin: bool,
) -> Result<(), JSONRPCErrorError> {
    if !stream_stdin {
        return Err(invalid_request(
            "stdin streaming is not enabled for this command/exec".to_string(),
        ));
    }
    if !delta.is_empty() {
        session
            .writer_sender()
            .send(delta)
            .await
            .map_err(|_| invalid_request("stdin is already closed".to_string()))?;
    }
    if close_stdin {
        session.close_stdin();
    }
    Ok(())
}

fn handle_process_resize(
    session: &ProcessHandle,
    size: TerminalSize,
) -> Result<(), JSONRPCErrorError> {
    session
        .resize(size)
        .map_err(|err| invalid_request(format!("failed to resize PTY: {err}")))
}

pub(crate) fn terminal_size_from_protocol(
    size: CommandExecTerminalSize,
) -> Result<TerminalSize, JSONRPCErrorError> {
    if size.rows == 0 || size.cols == 0 {
        return Err(invalid_params(
            "command/exec size rows and cols must be greater than 0".to_string(),
        ));
    }
    Ok(TerminalSize {
        rows: size.rows,
        cols: size.cols,
    })
}

fn command_no_longer_running_error(process_id: &InternalProcessId) -> JSONRPCErrorError {
    invalid_request(format!(
        "command/exec {} is no longer running",
        process_id.error_repr(),
    ))
}

fn invalid_request(message: String) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: INVALID_REQUEST_ERROR_CODE,
        message,
        data: None,
    }
}

fn invalid_params(message: String) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: INVALID_PARAMS_ERROR_CODE,
        message,
        data: None,
    }
}

fn internal_error(message: String) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: INTERNAL_ERROR_CODE,
        message,
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use codex_protocol::config_types::WindowsSandboxLevel;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use codex_protocol::protocol::ReadOnlyAccess;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_utils_cargo_bin::cargo_bin;
    use pretty_assertions::assert_eq;
    use tokio::time::Duration;
    use tokio::time::timeout;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::outgoing_message::OutgoingEnvelope;
    use crate::outgoing_message::OutgoingMessage;

    fn test_outgoing() -> Arc<OutgoingMessageSender> {
        let (tx, _rx) = mpsc::channel(4);
        Arc::new(OutgoingMessageSender::new(tx))
    }

    fn windows_sandbox_exec_request() -> ExecRequest {
        let sandbox_policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::FullAccess,
            network_access: false,
        };
        ExecRequest {
            command: vec!["cmd".to_string()],
            cwd: PathBuf::from("."),
            env: HashMap::new(),
            network: None,
            expiration: ExecExpiration::DefaultTimeout,
            sandbox: SandboxType::WindowsRestrictedToken,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            sandbox_permissions: codex_core::sandboxing::SandboxPermissions::UseDefault,
            sandbox_policy: sandbox_policy.clone(),
            file_system_sandbox_policy: FileSystemSandboxPolicy::from(&sandbox_policy),
            network_sandbox_policy: NetworkSandboxPolicy::from(&sandbox_policy),
            justification: None,
            arg0: None,
        }
    }

    #[tokio::test]
    async fn windows_sandbox_streaming_exec_is_rejected() {
        let (tx, _rx) = mpsc::channel(1);
        let manager = CommandExecManager::default();
        let err = manager
            .start(StartCommandExecParams {
                outgoing: Arc::new(OutgoingMessageSender::new(tx)),
                request_id: ConnectionRequestId {
                    connection_id: ConnectionId(1),
                    request_id: codex_app_server_protocol::RequestId::Integer(42),
                },
                process_id: Some("proc-42".to_string()),
                exec_request: windows_sandbox_exec_request(),
                started_network_proxy: None,
                tty: false,
                stream_stdin: false,
                stream_stdout_stderr: true,
                output_bytes_cap: None,
                size: None,
            })
            .await
            .expect_err("streaming windows sandbox exec should be rejected");

        assert_eq!(err.code, INVALID_REQUEST_ERROR_CODE);
        assert_eq!(
            err.message,
            "streaming command/exec is not supported with windows sandbox"
        );
    }

    #[tokio::test]
    async fn windows_sandbox_disable_timeout_is_rejected() {
        let (tx, _rx) = mpsc::channel(1);
        let manager = CommandExecManager::default();
        let err = manager
            .start(StartCommandExecParams {
                outgoing: Arc::new(OutgoingMessageSender::new(tx)),
                request_id: ConnectionRequestId {
                    connection_id: ConnectionId(14),
                    request_id: codex_app_server_protocol::RequestId::Integer(43),
                },
                process_id: Some("proc-43".to_string()),
                exec_request: ExecRequest {
                    expiration: ExecExpiration::Cancellation(CancellationToken::new()),
                    ..windows_sandbox_exec_request()
                },
                started_network_proxy: None,
                tty: false,
                stream_stdin: false,
                stream_stdout_stderr: false,
                output_bytes_cap: Some(DEFAULT_OUTPUT_BYTES_CAP),
                size: None,
            })
            .await
            .expect_err("disableTimeout windows sandbox exec should be rejected");

        assert_eq!(err.code, INVALID_REQUEST_ERROR_CODE);
        assert_eq!(
            err.message,
            "disableTimeout is not supported with windows sandbox"
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[tokio::test]
    async fn windows_sandbox_non_streaming_exec_uses_execution_path() {
        let (tx, mut rx) = mpsc::channel(1);
        let manager = CommandExecManager::default();
        let request_id = ConnectionRequestId {
            connection_id: ConnectionId(7),
            request_id: codex_app_server_protocol::RequestId::Integer(99),
        };

        manager
            .start(StartCommandExecParams {
                outgoing: Arc::new(OutgoingMessageSender::new(tx)),
                request_id: request_id.clone(),
                process_id: Some("proc-99".to_string()),
                exec_request: windows_sandbox_exec_request(),
                started_network_proxy: None,
                tty: false,
                stream_stdin: false,
                stream_stdout_stderr: false,
                output_bytes_cap: Some(DEFAULT_OUTPUT_BYTES_CAP),
                size: None,
            })
            .await
            .expect("non-streaming windows sandbox exec should start");

        let envelope = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timed out waiting for outgoing message")
            .expect("channel closed before outgoing message");
        let OutgoingEnvelope::ToConnection {
            connection_id,
            message,
        } = envelope
        else {
            panic!("expected connection-scoped outgoing message");
        };
        assert_eq!(connection_id, request_id.connection_id);
        let OutgoingMessage::Error(error) = message else {
            panic!("expected execution failure to be reported as an error");
        };
        assert_eq!(error.id, request_id.request_id);
        assert!(error.error.message.starts_with("exec failed:"));
    }

    #[cfg(not(target_os = "windows"))]
    #[tokio::test]
    async fn cancellation_expiration_keeps_process_alive_until_terminated() {
        let (tx, mut rx) = mpsc::channel(4);
        let manager = CommandExecManager::default();
        let request_id = ConnectionRequestId {
            connection_id: ConnectionId(8),
            request_id: codex_app_server_protocol::RequestId::Integer(100),
        };
        let sandbox_policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::FullAccess,
            network_access: false,
        };

        manager
            .start(StartCommandExecParams {
                outgoing: Arc::new(OutgoingMessageSender::new(tx)),
                request_id: request_id.clone(),
                process_id: Some("proc-100".to_string()),
                exec_request: ExecRequest {
                    command: vec!["sh".to_string(), "-lc".to_string(), "sleep 30".to_string()],
                    cwd: PathBuf::from("."),
                    env: HashMap::new(),
                    network: None,
                    expiration: ExecExpiration::Cancellation(CancellationToken::new()),
                    sandbox: SandboxType::None,
                    windows_sandbox_level: WindowsSandboxLevel::Disabled,
                    sandbox_permissions: codex_core::sandboxing::SandboxPermissions::UseDefault,
                    sandbox_policy: sandbox_policy.clone(),
                    file_system_sandbox_policy: FileSystemSandboxPolicy::from(&sandbox_policy),
                    network_sandbox_policy: NetworkSandboxPolicy::from(&sandbox_policy),
                    justification: None,
                    arg0: None,
                },
                started_network_proxy: None,
                tty: false,
                stream_stdin: false,
                stream_stdout_stderr: false,
                output_bytes_cap: Some(DEFAULT_OUTPUT_BYTES_CAP),
                size: None,
            })
            .await
            .expect("cancellation-based exec should start");

        assert!(
            timeout(Duration::from_millis(250), rx.recv())
                .await
                .is_err(),
            "command/exec should remain active until explicit termination",
        );

        manager
            .terminate(
                request_id.clone(),
                CommandExecTerminateParams {
                    process_id: "proc-100".to_string(),
                },
            )
            .await
            .expect("terminate should succeed");

        let envelope = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timed out waiting for outgoing message")
            .expect("channel closed before outgoing message");
        let OutgoingEnvelope::ToConnection {
            connection_id,
            message,
        } = envelope
        else {
            panic!("expected connection-scoped outgoing message");
        };
        assert_eq!(connection_id, request_id.connection_id);
        let OutgoingMessage::Response(response) = message else {
            panic!("expected execution response after termination");
        };
        assert_eq!(response.id, request_id.request_id);
        let response: CommandExecResponse =
            serde_json::from_value(response.result).expect("deserialize command/exec response");
        assert_ne!(response.exit_code, 0);
        assert_eq!(response.stdout, "");
        // The deferred response now drains any already-emitted stderr before
        // replying, so shell startup noise is allowed here.
    }

    #[tokio::test]
    async fn windows_sandbox_process_ids_reject_write_requests() {
        let manager = CommandExecManager::default();
        let request_id = ConnectionRequestId {
            connection_id: ConnectionId(11),
            request_id: codex_app_server_protocol::RequestId::Integer(1),
        };
        let process_id = ConnectionProcessId {
            connection_id: request_id.connection_id,
            process_id: InternalProcessId::Client("proc-11".to_string()),
        };
        manager
            .sessions
            .lock()
            .await
            .insert(process_id, CommandExecSession::UnsupportedWindowsSandbox);

        let err = manager
            .write(
                request_id,
                CommandExecWriteParams {
                    process_id: "proc-11".to_string(),
                    delta_base64: Some(STANDARD.encode("hello")),
                    close_stdin: false,
                },
            )
            .await
            .expect_err("windows sandbox process ids should reject command/exec/write");

        assert_eq!(err.code, INVALID_REQUEST_ERROR_CODE);
        assert_eq!(
            err.message,
            "command/exec/write, command/exec/terminate, and command/exec/resize are not supported for windows sandbox processes"
        );
    }

    #[tokio::test]
    async fn windows_sandbox_process_ids_reject_terminate_requests() {
        let manager = CommandExecManager::default();
        let request_id = ConnectionRequestId {
            connection_id: ConnectionId(12),
            request_id: codex_app_server_protocol::RequestId::Integer(2),
        };
        let process_id = ConnectionProcessId {
            connection_id: request_id.connection_id,
            process_id: InternalProcessId::Client("proc-12".to_string()),
        };
        manager
            .sessions
            .lock()
            .await
            .insert(process_id, CommandExecSession::UnsupportedWindowsSandbox);

        let err = manager
            .terminate(
                request_id,
                CommandExecTerminateParams {
                    process_id: "proc-12".to_string(),
                },
            )
            .await
            .expect_err("windows sandbox process ids should reject command/exec/terminate");

        assert_eq!(err.code, INVALID_REQUEST_ERROR_CODE);
        assert_eq!(
            err.message,
            "command/exec/write, command/exec/terminate, and command/exec/resize are not supported for windows sandbox processes"
        );
    }

    #[tokio::test]
    async fn dropped_control_request_is_reported_as_not_running() {
        let manager = CommandExecManager::default();
        let request_id = ConnectionRequestId {
            connection_id: ConnectionId(13),
            request_id: codex_app_server_protocol::RequestId::Integer(3),
        };
        let process_id = InternalProcessId::Client("proc-13".to_string());
        let (control_tx, mut control_rx) = mpsc::channel(1);
        manager.sessions.lock().await.insert(
            ConnectionProcessId {
                connection_id: request_id.connection_id,
                process_id: process_id.clone(),
            },
            CommandExecSession::Active { control_tx },
        );

        tokio::spawn(async move {
            let _request = control_rx
                .recv()
                .await
                .expect("expected queued control request");
        });

        let err = manager
            .terminate(
                request_id,
                CommandExecTerminateParams {
                    process_id: "proc-13".to_string(),
                },
            )
            .await
            .expect_err("dropped control request should be treated as not running");

        assert_eq!(err.code, INVALID_REQUEST_ERROR_CODE);
        assert_eq!(err.message, "command/exec \"proc-13\" is no longer running");
    }

    #[tokio::test]
    async fn process_output_drains_ready_bytes_even_after_quiet_deadline_elapsed() {
        let (output_tx, output_rx) = mpsc::channel(4);
        output_tx
            .send(b"tail".to_vec())
            .await
            .expect("queue ready output");
        let (process_exited_tx, process_exited_rx) = watch::channel(true);
        let output = collect_process_output(
            SpawnProcessOutputParams {
                connection_id: ConnectionId(21),
                process_id: None,
                output_rx,
                process_exited_rx,
                outgoing: test_outgoing(),
                stream: CommandExecOutputStream::Stdout,
                stream_output: false,
                output_bytes_cap: None,
            },
            Duration::ZERO,
            Duration::from_millis(1),
        )
        .await;

        drop(process_exited_tx);
        assert_eq!(output, "tail");
    }

    #[tokio::test]
    async fn process_output_resets_quiet_window_when_more_tail_bytes_arrive() {
        let (output_tx, output_rx) = mpsc::channel(4);
        let (process_exited_tx, process_exited_rx) = watch::channel(false);

        let send_task = tokio::spawn(async move {
            process_exited_tx
                .send(true)
                .expect("signal process exit to collector");
            tokio::time::sleep(Duration::from_millis(10)).await;
            output_tx
                .send(b"one".to_vec())
                .await
                .expect("send first trailing chunk");
            tokio::time::sleep(Duration::from_millis(10)).await;
            output_tx
                .send(b"two".to_vec())
                .await
                .expect("send second trailing chunk");
        });

        let output = collect_process_output(
            SpawnProcessOutputParams {
                connection_id: ConnectionId(22),
                process_id: None,
                output_rx,
                process_exited_rx,
                outgoing: test_outgoing(),
                stream: CommandExecOutputStream::Stdout,
                stream_output: false,
                output_bytes_cap: None,
            },
            Duration::from_millis(15),
            Duration::from_millis(100),
        )
        .await;

        send_task.await.expect("sender task should complete");
        assert_eq!(output, "onetwo");
    }

    #[tokio::test]
    async fn process_output_stops_after_hard_cap_when_stream_stays_open() {
        let (_output_tx, output_rx) = mpsc::channel(4);
        let (process_exited_tx, process_exited_rx) = watch::channel(true);
        let output = timeout(
            Duration::from_millis(100),
            collect_process_output(
                SpawnProcessOutputParams {
                    connection_id: ConnectionId(23),
                    process_id: None,
                    output_rx,
                    process_exited_rx,
                    outgoing: test_outgoing(),
                    stream: CommandExecOutputStream::Stdout,
                    stream_output: false,
                    output_bytes_cap: None,
                },
                Duration::from_millis(10),
                Duration::from_millis(20),
            ),
        )
        .await
        .expect("collector should stop without waiting forever");

        drop(process_exited_tx);
        assert_eq!(output, "");
    }

    #[tokio::test]
    async fn run_command_closes_control_channel_before_sending_drained_response() {
        enum ObservedEvent {
            ControlClosed,
            Response(OutgoingEnvelope),
        }

        let helper = cargo_bin("codex-app-server-command-exec-test-helper")
            .expect("should find command_exec test helper");
        let spawned = codex_utils_pty::spawn_pipe_process_no_stdin(
            &helper.to_string_lossy(),
            &Vec::new(),
            PathBuf::from(".").as_path(),
            &HashMap::new(),
            &None,
        )
        .await
        .expect("helper process should spawn");
        let (control_tx, control_rx) = mpsc::channel(4);
        let (tx, mut rx) = mpsc::channel(4);
        let request_id = ConnectionRequestId {
            connection_id: ConnectionId(24),
            request_id: codex_app_server_protocol::RequestId::Integer(4),
        };

        let run_task = tokio::spawn(run_command(RunCommandParams {
            outgoing: Arc::new(OutgoingMessageSender::new(tx)),
            request_id: request_id.clone(),
            process_id: Some("proc-24".to_string()),
            spawned,
            control_rx,
            stream_stdin: false,
            stream_stdout_stderr: false,
            expiration: ExecExpiration::DefaultTimeout,
            output_bytes_cap: None,
        }));

        let (event_tx, mut event_rx) = mpsc::channel(2);
        let closed_event_tx = event_tx.clone();
        let closed_control_tx = control_tx.clone();
        let closed_observer = tokio::spawn(async move {
            closed_control_tx.closed().await;
            let _ = closed_event_tx.send(ObservedEvent::ControlClosed).await;
        });
        let response_observer = tokio::spawn(async move {
            if let Some(envelope) = rx.recv().await {
                let _ = event_tx.send(ObservedEvent::Response(envelope)).await;
            }
        });

        let first_event = timeout(Duration::from_secs(1), event_rx.recv())
            .await
            .expect("timed out waiting for control close or drained response")
            .expect("observer channel closed before first event");
        assert!(
            matches!(first_event, ObservedEvent::ControlClosed),
            "drained response arrived before control receiver closed",
        );
        let (response_tx, _response_rx) = oneshot::channel();
        let send_result = control_tx
            .send(CommandControlRequest {
                control: CommandControl::Terminate,
                response_tx: Some(response_tx),
            })
            .await;
        assert!(send_result.is_err(), "post-exit control send should fail");

        let second_event = timeout(Duration::from_secs(1), event_rx.recv())
            .await
            .expect("timed out waiting for drained response")
            .expect("observer channel closed before drained response");
        let ObservedEvent::Response(envelope) = second_event else {
            panic!("expected drained response after control receiver closed");
        };
        let OutgoingEnvelope::ToConnection {
            connection_id,
            message,
        } = envelope
        else {
            panic!("expected connection-scoped outgoing message");
        };
        assert_eq!(connection_id, request_id.connection_id);
        let OutgoingMessage::Response(response) = message else {
            panic!("expected command/exec response after drain");
        };
        assert_eq!(response.id, request_id.request_id);
        let response: CommandExecResponse =
            serde_json::from_value(response.result).expect("deserialize command/exec response");
        assert_eq!(
            response,
            CommandExecResponse {
                exit_code: 0,
                stdout: "tail".to_string(),
                stderr: String::new(),
            }
        );

        run_task.await.expect("run_command task should complete");
        closed_observer
            .await
            .expect("closed observer task should complete");
        response_observer
            .await
            .expect("response observer task should complete");
    }
}
