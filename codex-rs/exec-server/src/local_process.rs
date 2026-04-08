use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use async_trait::async_trait;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxExecRequest;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxType;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_pty::ExecCommandSession;
use codex_utils_pty::TerminalSize;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::sync::watch;

use crate::ExecBackend;
use crate::ExecProcess;
use crate::ExecServerError;
use crate::ProcessId;
use crate::StartedExecProcess;
use crate::protocol::EXEC_CLOSED_METHOD;
use crate::protocol::ExecClosedNotification;
use crate::protocol::ExecExitedNotification;
use crate::protocol::ExecOutputDeltaNotification;
use crate::protocol::ExecOutputStream;
use crate::protocol::ExecParams;
use crate::protocol::ExecResponse;
use crate::protocol::InitializeResponse;
use crate::protocol::ProcessOutputChunk;
use crate::protocol::ReadParams;
use crate::protocol::ReadResponse;
use crate::protocol::TerminateParams;
use crate::protocol::TerminateResponse;
use crate::protocol::WriteParams;
use crate::protocol::WriteResponse;
use crate::protocol::WriteStatus;
use crate::rpc::RpcNotificationSender;
use crate::rpc::RpcServerOutboundMessage;
use crate::rpc::internal_error;
use crate::rpc::invalid_params;
use crate::rpc::invalid_request;

const RETAINED_OUTPUT_BYTES_PER_PROCESS: usize = 1024 * 1024;
const NOTIFICATION_CHANNEL_CAPACITY: usize = 256;
#[cfg(test)]
const EXITED_PROCESS_RETENTION: Duration = Duration::from_millis(25);
#[cfg(not(test))]
const EXITED_PROCESS_RETENTION: Duration = Duration::from_secs(30);

#[derive(Clone)]
struct RetainedOutputChunk {
    seq: u64,
    stream: ExecOutputStream,
    chunk: Vec<u8>,
}

struct RunningProcess {
    session: ExecCommandSession,
    tty: bool,
    output: VecDeque<RetainedOutputChunk>,
    retained_bytes: usize,
    next_seq: u64,
    exit_code: Option<i32>,
    wake_tx: watch::Sender<u64>,
    output_notify: Arc<Notify>,
    open_streams: usize,
    closed: bool,
}

enum ProcessEntry {
    Starting,
    Running(Box<RunningProcess>),
}

struct Inner {
    notifications: RpcNotificationSender,
    processes: Mutex<HashMap<ProcessId, ProcessEntry>>,
    initialize_requested: AtomicBool,
    initialized: AtomicBool,
    runtime: ExecServerRuntimeConfig,
}

#[derive(Clone)]
pub(crate) struct LocalProcess {
    inner: Arc<Inner>,
}

struct LocalExecProcess {
    process_id: ProcessId,
    backend: LocalProcess,
    wake_tx: watch::Sender<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct ExecServerRuntimeConfig {
    codex_linux_sandbox_exe: Option<PathBuf>,
}

impl ExecServerRuntimeConfig {
    pub fn new(codex_linux_sandbox_exe: Option<PathBuf>) -> Self {
        Self {
            codex_linux_sandbox_exe,
        }
    }

    pub fn detect() -> Self {
        Self::default()
    }
}

struct StartedProcess {
    process_id: ProcessId,
    sandbox_type: SandboxType,
    wake_tx: watch::Sender<u64>,
}

impl Default for LocalProcess {
    fn default() -> Self {
        Self::default_with_runtime(ExecServerRuntimeConfig::detect())
    }
}

impl LocalProcess {
    pub(crate) fn default_with_runtime(runtime: ExecServerRuntimeConfig) -> Self {
        let (outgoing_tx, mut outgoing_rx) =
            mpsc::channel::<RpcServerOutboundMessage>(NOTIFICATION_CHANNEL_CAPACITY);
        tokio::spawn(async move { while outgoing_rx.recv().await.is_some() {} });
        Self::new_with_runtime(RpcNotificationSender::new(outgoing_tx), runtime)
    }
}

impl LocalProcess {
    pub(crate) fn new_with_runtime(
        notifications: RpcNotificationSender,
        runtime: ExecServerRuntimeConfig,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                notifications,
                processes: Mutex::new(HashMap::new()),
                initialize_requested: AtomicBool::new(false),
                initialized: AtomicBool::new(false),
                runtime,
            }),
        }
    }

    pub(crate) async fn shutdown(&self) {
        let remaining = {
            let mut processes = self.inner.processes.lock().await;
            processes
                .drain()
                .filter_map(|(_, process)| match process {
                    ProcessEntry::Starting => None,
                    ProcessEntry::Running(process) => Some(process),
                })
                .collect::<Vec<_>>()
        };
        for process in remaining {
            process.session.terminate();
        }
    }

    pub(crate) fn initialize(&self) -> Result<InitializeResponse, JSONRPCErrorError> {
        if self.inner.initialize_requested.swap(true, Ordering::SeqCst) {
            return Err(invalid_request(
                "initialize may only be sent once per connection".to_string(),
            ));
        }
        Ok(InitializeResponse {})
    }

    pub(crate) fn initialized(&self) -> Result<(), String> {
        if !self.inner.initialize_requested.load(Ordering::SeqCst) {
            return Err("received `initialized` notification before `initialize`".into());
        }
        self.inner.initialized.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub(crate) fn require_initialized_for(
        &self,
        method_family: &str,
    ) -> Result<(), JSONRPCErrorError> {
        if !self.inner.initialize_requested.load(Ordering::SeqCst) {
            return Err(invalid_request(format!(
                "client must call initialize before using {method_family} methods"
            )));
        }
        if !self.inner.initialized.load(Ordering::SeqCst) {
            return Err(invalid_request(format!(
                "client must send initialized before using {method_family} methods"
            )));
        }
        Ok(())
    }

    async fn start_process(&self, params: ExecParams) -> Result<StartedProcess, JSONRPCErrorError> {
        self.require_initialized_for("exec")?;
        let process_id = params.process_id.clone();
        let launch = prepare_exec_launch(&params, &self.inner.runtime)?;
        let (program, args) = launch
            .command
            .split_first()
            .ok_or_else(|| invalid_params("argv must not be empty".to_string()))?;

        {
            let mut process_map = self.inner.processes.lock().await;
            if process_map.contains_key(&process_id) {
                return Err(invalid_request(format!(
                    "process {process_id} already exists"
                )));
            }
            process_map.insert(process_id.clone(), ProcessEntry::Starting);
        }

        let spawned_result = if params.tty {
            codex_utils_pty::spawn_pty_process(
                program,
                args,
                params.cwd.as_path(),
                &launch.env,
                &launch.arg0,
                TerminalSize::default(),
            )
            .await
        } else {
            codex_utils_pty::spawn_pipe_process_no_stdin(
                program,
                args,
                params.cwd.as_path(),
                &launch.env,
                &launch.arg0,
            )
            .await
        };
        let spawned = match spawned_result {
            Ok(spawned) => spawned,
            Err(err) => {
                let mut process_map = self.inner.processes.lock().await;
                if matches!(process_map.get(&process_id), Some(ProcessEntry::Starting)) {
                    process_map.remove(&process_id);
                }
                return Err(internal_error(err.to_string()));
            }
        };

        let output_notify = Arc::new(Notify::new());
        let (wake_tx, _wake_rx) = watch::channel(0);
        {
            let mut process_map = self.inner.processes.lock().await;
            process_map.insert(
                process_id.clone(),
                ProcessEntry::Running(Box::new(RunningProcess {
                    session: spawned.session,
                    tty: params.tty,
                    output: VecDeque::new(),
                    retained_bytes: 0,
                    next_seq: 1,
                    exit_code: None,
                    wake_tx: wake_tx.clone(),
                    output_notify: Arc::clone(&output_notify),
                    open_streams: 2,
                    closed: false,
                })),
            );
        }

        tokio::spawn(stream_output(
            process_id.clone(),
            if params.tty {
                ExecOutputStream::Pty
            } else {
                ExecOutputStream::Stdout
            },
            spawned.stdout_rx,
            Arc::clone(&self.inner),
            Arc::clone(&output_notify),
        ));
        tokio::spawn(stream_output(
            process_id.clone(),
            if params.tty {
                ExecOutputStream::Pty
            } else {
                ExecOutputStream::Stderr
            },
            spawned.stderr_rx,
            Arc::clone(&self.inner),
            Arc::clone(&output_notify),
        ));
        tokio::spawn(watch_exit(
            process_id.clone(),
            spawned.exit_rx,
            Arc::clone(&self.inner),
            output_notify,
        ));

        Ok(StartedProcess {
            process_id,
            sandbox_type: launch.sandbox,
            wake_tx,
        })
    }

    pub(crate) async fn exec(&self, params: ExecParams) -> Result<ExecResponse, JSONRPCErrorError> {
        self.start_process(params)
            .await
            .map(|started| ExecResponse {
                process_id: started.process_id,
                sandbox: started.sandbox_type,
            })
    }

    pub(crate) async fn exec_read(
        &self,
        params: ReadParams,
    ) -> Result<ReadResponse, JSONRPCErrorError> {
        self.require_initialized_for("exec")?;
        let _process_id = params.process_id.clone();
        let after_seq = params.after_seq.unwrap_or(0);
        let max_bytes = params.max_bytes.unwrap_or(usize::MAX);
        let wait = Duration::from_millis(params.wait_ms.unwrap_or(0));
        let deadline = tokio::time::Instant::now() + wait;

        loop {
            let (response, output_notify) = {
                let process_map = self.inner.processes.lock().await;
                let process = process_map.get(&params.process_id).ok_or_else(|| {
                    invalid_request(format!("unknown process id {}", params.process_id))
                })?;
                let ProcessEntry::Running(process) = process else {
                    return Err(invalid_request(format!(
                        "process id {} is starting",
                        params.process_id
                    )));
                };

                let mut chunks = Vec::new();
                let mut total_bytes = 0;
                let mut next_seq = process.next_seq;
                for retained in process.output.iter().filter(|chunk| chunk.seq > after_seq) {
                    let chunk_len = retained.chunk.len();
                    if !chunks.is_empty() && total_bytes + chunk_len > max_bytes {
                        break;
                    }
                    total_bytes += chunk_len;
                    chunks.push(ProcessOutputChunk {
                        seq: retained.seq,
                        stream: retained.stream,
                        chunk: retained.chunk.clone().into(),
                    });
                    next_seq = retained.seq + 1;
                    if total_bytes >= max_bytes {
                        break;
                    }
                }

                (
                    ReadResponse {
                        chunks,
                        next_seq,
                        exited: process.exit_code.is_some(),
                        exit_code: process.exit_code,
                        closed: process.closed,
                        failure: None,
                    },
                    Arc::clone(&process.output_notify),
                )
            };

            if !response.chunks.is_empty()
                || response.exited
                || tokio::time::Instant::now() >= deadline
            {
                let _total_bytes: usize = response
                    .chunks
                    .iter()
                    .map(|chunk| chunk.chunk.0.len())
                    .sum();
                return Ok(response);
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Ok(response);
            }
            let _ = tokio::time::timeout(remaining, output_notify.notified()).await;
        }
    }

    pub(crate) async fn exec_write(
        &self,
        params: WriteParams,
    ) -> Result<WriteResponse, JSONRPCErrorError> {
        self.require_initialized_for("exec")?;
        let _process_id = params.process_id.clone();
        let _input_bytes = params.chunk.0.len();
        let writer_tx = {
            let process_map = self.inner.processes.lock().await;
            let Some(process) = process_map.get(&params.process_id) else {
                return Ok(WriteResponse {
                    status: WriteStatus::UnknownProcess,
                });
            };
            let ProcessEntry::Running(process) = process else {
                return Ok(WriteResponse {
                    status: WriteStatus::Starting,
                });
            };
            if !process.tty {
                return Ok(WriteResponse {
                    status: WriteStatus::StdinClosed,
                });
            }
            process.session.writer_sender()
        };

        writer_tx
            .send(params.chunk.into_inner())
            .await
            .map_err(|_| internal_error("failed to write to process stdin".to_string()))?;

        Ok(WriteResponse {
            status: WriteStatus::Accepted,
        })
    }

    pub(crate) async fn terminate_process(
        &self,
        params: TerminateParams,
    ) -> Result<TerminateResponse, JSONRPCErrorError> {
        self.require_initialized_for("exec")?;
        let _process_id = params.process_id.clone();
        let running = {
            let process_map = self.inner.processes.lock().await;
            match process_map.get(&params.process_id) {
                Some(ProcessEntry::Running(process)) => {
                    if process.exit_code.is_some() {
                        return Ok(TerminateResponse { running: false });
                    }
                    process.session.terminate();
                    true
                }
                Some(ProcessEntry::Starting) | None => false,
            }
        };

        Ok(TerminateResponse { running })
    }
}

#[async_trait]
impl ExecBackend for LocalProcess {
    async fn start(&self, params: ExecParams) -> Result<StartedExecProcess, ExecServerError> {
        let started = self
            .start_process(params)
            .await
            .map_err(map_handler_error)?;
        Ok(StartedExecProcess {
            process: Arc::new(LocalExecProcess {
                process_id: started.process_id,
                backend: self.clone(),
                wake_tx: started.wake_tx,
            }),
            sandbox_type: started.sandbox_type,
        })
    }
}

#[async_trait]
impl ExecProcess for LocalExecProcess {
    fn process_id(&self) -> &ProcessId {
        &self.process_id
    }

    fn subscribe_wake(&self) -> watch::Receiver<u64> {
        self.wake_tx.subscribe()
    }

    async fn read(
        &self,
        after_seq: Option<u64>,
        max_bytes: Option<usize>,
        wait_ms: Option<u64>,
    ) -> Result<ReadResponse, ExecServerError> {
        self.backend
            .read(&self.process_id, after_seq, max_bytes, wait_ms)
            .await
    }

    async fn write(&self, chunk: Vec<u8>) -> Result<WriteResponse, ExecServerError> {
        self.backend.write(&self.process_id, chunk).await
    }

    async fn terminate(&self) -> Result<(), ExecServerError> {
        self.backend.terminate(&self.process_id).await
    }
}

fn prepare_exec_launch(
    params: &ExecParams,
    runtime: &ExecServerRuntimeConfig,
) -> Result<SandboxExecRequest, JSONRPCErrorError> {
    let (program, args) = params
        .argv
        .split_first()
        .ok_or_else(|| invalid_params("argv must not be empty".to_string()))?;
    let command = SandboxCommand {
        program: program.clone().into(),
        args: args.to_vec(),
        cwd: AbsolutePathBuf::try_from(params.cwd.as_path())
            .map_err(|err| invalid_params(format!("cwd must be absolute: {err}")))?,
        env: params.env.clone(),
        additional_permissions: params.sandbox.additional_permissions.clone(),
    };
    SandboxManager::new()
        .transform(
            command,
            &params.sandbox,
            // TODO: Thread managed-network proxy state across exec-server so
            // sandbox profile generation preserves proxy-specific allowances.
            /*network*/
            None,
            runtime.codex_linux_sandbox_exe.as_deref(),
        )
        .map_err(|err| internal_error(format!("failed to build sandbox launch: {err}")))
}

impl LocalProcess {
    async fn read(
        &self,
        process_id: &ProcessId,
        after_seq: Option<u64>,
        max_bytes: Option<usize>,
        wait_ms: Option<u64>,
    ) -> Result<ReadResponse, ExecServerError> {
        self.exec_read(ReadParams {
            process_id: process_id.clone(),
            after_seq,
            max_bytes,
            wait_ms,
        })
        .await
        .map_err(map_handler_error)
    }

    async fn write(
        &self,
        process_id: &ProcessId,
        chunk: Vec<u8>,
    ) -> Result<WriteResponse, ExecServerError> {
        self.exec_write(WriteParams {
            process_id: process_id.clone(),
            chunk: chunk.into(),
        })
        .await
        .map_err(map_handler_error)
    }

    async fn terminate(&self, process_id: &ProcessId) -> Result<(), ExecServerError> {
        self.terminate_process(TerminateParams {
            process_id: process_id.clone(),
        })
        .await
        .map_err(map_handler_error)?;
        Ok(())
    }
}

fn map_handler_error(error: JSONRPCErrorError) -> ExecServerError {
    ExecServerError::Server {
        code: error.code,
        message: error.message,
    }
}

async fn stream_output(
    process_id: ProcessId,
    stream: ExecOutputStream,
    mut receiver: tokio::sync::mpsc::Receiver<Vec<u8>>,
    inner: Arc<Inner>,
    output_notify: Arc<Notify>,
) {
    while let Some(chunk) = receiver.recv().await {
        let _chunk_len = chunk.len();
        let notification = {
            let mut processes = inner.processes.lock().await;
            let Some(entry) = processes.get_mut(&process_id) else {
                break;
            };
            let ProcessEntry::Running(process) = entry else {
                break;
            };
            let seq = process.next_seq;
            process.next_seq += 1;
            process.retained_bytes += chunk.len();
            process.output.push_back(RetainedOutputChunk {
                seq,
                stream,
                chunk: chunk.clone(),
            });
            while process.retained_bytes > RETAINED_OUTPUT_BYTES_PER_PROCESS {
                let Some(evicted) = process.output.pop_front() else {
                    break;
                };
                process.retained_bytes = process.retained_bytes.saturating_sub(evicted.chunk.len());
            }
            let _ = process.wake_tx.send(seq);
            ExecOutputDeltaNotification {
                process_id: process_id.clone(),
                seq,
                stream,
                chunk: chunk.into(),
            }
        };
        output_notify.notify_waiters();
        if inner
            .notifications
            .notify(crate::protocol::EXEC_OUTPUT_DELTA_METHOD, &notification)
            .await
            .is_err()
        {
            break;
        }
    }

    finish_output_stream(process_id, inner).await;
}

async fn watch_exit(
    process_id: ProcessId,
    exit_rx: tokio::sync::oneshot::Receiver<i32>,
    inner: Arc<Inner>,
    output_notify: Arc<Notify>,
) {
    let exit_code = exit_rx.await.unwrap_or(-1);
    let notification = {
        let mut processes = inner.processes.lock().await;
        if let Some(ProcessEntry::Running(process)) = processes.get_mut(&process_id) {
            let seq = process.next_seq;
            process.next_seq += 1;
            process.exit_code = Some(exit_code);
            let _ = process.wake_tx.send(seq);
            Some(ExecExitedNotification {
                process_id: process_id.clone(),
                seq,
                exit_code,
            })
        } else {
            None
        }
    };
    output_notify.notify_waiters();
    if let Some(notification) = notification
        && inner
            .notifications
            .notify(crate::protocol::EXEC_EXITED_METHOD, &notification)
            .await
            .is_err()
    {
        return;
    }

    maybe_emit_closed(process_id.clone(), Arc::clone(&inner)).await;

    tokio::time::sleep(EXITED_PROCESS_RETENTION).await;
    let mut processes = inner.processes.lock().await;
    if matches!(
        processes.get(&process_id),
        Some(ProcessEntry::Running(process)) if process.exit_code == Some(exit_code)
    ) {
        processes.remove(&process_id);
    }
}

async fn finish_output_stream(process_id: ProcessId, inner: Arc<Inner>) {
    {
        let mut processes = inner.processes.lock().await;
        let Some(ProcessEntry::Running(process)) = processes.get_mut(&process_id) else {
            return;
        };

        if process.open_streams > 0 {
            process.open_streams -= 1;
        }
    }

    maybe_emit_closed(process_id, inner).await;
}

async fn maybe_emit_closed(process_id: ProcessId, inner: Arc<Inner>) {
    let notification = {
        let mut processes = inner.processes.lock().await;
        let Some(ProcessEntry::Running(process)) = processes.get_mut(&process_id) else {
            return;
        };

        if process.closed || process.open_streams != 0 || process.exit_code.is_none() {
            return;
        }

        process.closed = true;
        let seq = process.next_seq;
        process.next_seq += 1;
        let _ = process.wake_tx.send(seq);
        Some(ExecClosedNotification {
            process_id: process_id.clone(),
            seq,
        })
    };

    let Some(notification) = notification else {
        return;
    };

    if inner
        .notifications
        .notify(EXEC_CLOSED_METHOD, &notification)
        .await
        .is_err()
    {}
}
