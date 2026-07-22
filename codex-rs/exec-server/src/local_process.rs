use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_exec_server_protocol::JSONRPCErrorError;
use codex_network_proxy::NetworkProxyHandle;
use codex_protocol::config_types::EnvironmentVariablePattern;
use codex_protocol::config_types::ShellEnvironmentPolicy;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::exec_output::StreamOutput;
use codex_protocol::shell_environment;
use codex_sandboxing::SandboxType;
use codex_sandboxing::is_likely_sandbox_denied;
use codex_utils_pty::ExecCommandSession;
use codex_utils_pty::ProcessSignal as PtyProcessSignal;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::ExecBackend;
use crate::ExecBackendFuture;
use crate::ExecProcess;
use crate::ExecProcessEvent;
use crate::ExecProcessEventReceiver;
use crate::ExecProcessFuture;
use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::ProcessId;
use crate::StartedExecProcess;
use crate::network_policy_decisions::network_policy_decider;
use crate::process::ExecProcessEventLog;
use crate::process_sandbox::prepare_exec_request;
use crate::protocol::EXEC_CLOSED_METHOD;
use crate::protocol::ExecClosedNotification;
use crate::protocol::ExecEnvPolicy;
use crate::protocol::ExecExitedNotification;
use crate::protocol::ExecOutputDeltaNotification;
use crate::protocol::ExecOutputStream;
use crate::protocol::ExecParams;
use crate::protocol::ExecResponse;
use crate::protocol::MAX_NETWORK_POLICY_PROCESS_ID_BYTES;
use crate::protocol::ProcessOutputChunk;
use crate::protocol::ProcessSignal;
use crate::protocol::ReadParams;
use crate::protocol::ReadResponse;
use crate::protocol::SignalParams;
use crate::protocol::SignalResponse;
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
use crate::rpc_server_requests::RpcServerRequestSender;
use crate::telemetry::ExecServerTelemetry;
use crate::telemetry::ProcessMetricGuard;

const RETAINED_OUTPUT_BYTES_PER_PROCESS: usize = 1024 * 1024;
// Each process/read chunk needs four JSON values. Keep retained replay below the
// shared 256K-value JSON-RPC decoder budget even when output arrives in tiny chunks.
const RETAINED_OUTPUT_CHUNKS_PER_PROCESS: usize = 50_000;
const NOTIFICATION_CHANNEL_CAPACITY: usize = 256;
const PROCESS_EVENT_CHANNEL_CAPACITY: usize = 256;
const RETAINED_STDIN_WRITE_IDS_PER_PROCESS: usize = 4096;
static NEXT_LOCAL_STDIN_WRITE_ID: AtomicU64 = AtomicU64::new(1);
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
    pipe_stdin: bool,
    accepted_stdin_write_ids: Arc<Mutex<AcceptedStdinWriteIds>>,
    output: VecDeque<RetainedOutputChunk>,
    retained_bytes: usize,
    next_seq: u64,
    exit_code: Option<i32>,
    wake_tx: watch::Sender<u64>,
    events: ExecProcessEventLog,
    output_notify: Arc<Notify>,
    open_streams: usize,
    closed: bool,
    metrics: Option<ProcessMetricGuard>,
    termination_requested: bool,
    sandbox: SandboxType,
    sandbox_denied: bool,
    network_proxy_handle: Option<NetworkProxyHandle>,
    network_policy_shutdown: Option<CancellationToken>,
}

/// Bounded cache of stdin write ids that have already been accepted for one process.
///
/// A remote client can retry `process/write` after reconnecting. Remembering accepted
/// ids lets the server acknowledge the retried request without writing the same bytes
/// to child stdin twice.
#[derive(Default)]
struct AcceptedStdinWriteIds {
    ids: HashSet<String>,
    order: VecDeque<String>,
}

impl AcceptedStdinWriteIds {
    fn contains(&self, write_id: &str) -> bool {
        self.ids.contains(write_id)
    }

    fn remember(&mut self, write_id: String) {
        if !self.ids.insert(write_id.clone()) {
            return;
        }

        self.order.push_back(write_id);
        while self.order.len() > RETAINED_STDIN_WRITE_IDS_PER_PROCESS {
            let Some(evicted) = self.order.pop_front() else {
                break;
            };
            self.ids.remove(&evicted);
        }
    }
}

struct ProcessStart;

enum ProcessEntry {
    Starting(Arc<ProcessStart>),
    Running(Box<RunningProcess>),
}

struct Inner {
    notifications: std::sync::RwLock<Option<RpcNotificationSender>>,
    requests: Arc<std::sync::RwLock<Option<RpcServerRequestSender>>>,
    processes: Mutex<HashMap<ProcessId, ProcessEntry>>,
    telemetry: ExecServerTelemetry,
}

#[derive(Clone)]
pub(crate) struct LocalProcess {
    inner: Arc<Inner>,
    runtime_paths: Option<ExecServerRuntimePaths>,
}

struct LocalExecProcess {
    process_id: ProcessId,
    backend: LocalProcess,
    wake_tx: watch::Sender<u64>,
    events: ExecProcessEventLog,
}

impl Default for LocalProcess {
    fn default() -> Self {
        Self::with_discarded_notifications(/*runtime_paths*/ None)
    }
}

impl LocalProcess {
    pub(crate) fn with_local_runtime_paths(runtime_paths: ExecServerRuntimePaths) -> Self {
        Self::with_discarded_notifications(Some(runtime_paths))
    }

    fn with_discarded_notifications(runtime_paths: Option<ExecServerRuntimePaths>) -> Self {
        let (outgoing_tx, mut outgoing_rx) =
            mpsc::channel::<RpcServerOutboundMessage>(NOTIFICATION_CHANNEL_CAPACITY);
        tokio::spawn(async move { while outgoing_rx.recv().await.is_some() {} });
        Self::with_runtime_paths(
            RpcNotificationSender::new(outgoing_tx),
            ExecServerTelemetry::default(),
            runtime_paths,
        )
    }

    pub(crate) fn new(
        notifications: RpcNotificationSender,
        telemetry: ExecServerTelemetry,
        runtime_paths: ExecServerRuntimePaths,
    ) -> Self {
        Self::with_runtime_paths(notifications, telemetry, Some(runtime_paths))
    }

    fn with_runtime_paths(
        notifications: RpcNotificationSender,
        telemetry: ExecServerTelemetry,
        runtime_paths: Option<ExecServerRuntimePaths>,
    ) -> Self {
        let requests = notifications.request_sender();
        Self {
            inner: Arc::new(Inner {
                notifications: std::sync::RwLock::new(Some(notifications)),
                requests: Arc::new(std::sync::RwLock::new(Some(requests))),
                processes: Mutex::new(HashMap::new()),
                telemetry,
            }),
            runtime_paths,
        }
    }

    pub(crate) async fn shutdown(&self) {
        let remaining = {
            let mut processes = self.inner.processes.lock().await;
            processes
                .drain()
                .filter_map(|(_, process)| match process {
                    ProcessEntry::Starting(_) => None,
                    ProcessEntry::Running(process) => Some(process),
                })
                .collect::<Vec<_>>()
        };
        for mut process in remaining {
            if let Some(network_policy_shutdown) = process.network_policy_shutdown.take() {
                network_policy_shutdown.cancel();
            }
            if let Some(metrics) = process.metrics.take() {
                metrics.finish("terminated");
            }
            process.session.terminate();
        }
    }

    pub(crate) fn set_notification_sender(&self, notifications: Option<RpcNotificationSender>) {
        let requests = notifications
            .as_ref()
            .map(RpcNotificationSender::request_sender);
        let mut notification_sender = self
            .inner
            .notifications
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *notification_sender = notifications;
        let previous_requests = std::mem::replace(
            &mut *self
                .inner
                .requests
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            requests,
        );
        if let Some(previous_requests) = previous_requests {
            previous_requests.close();
        }
    }

    async fn start_process(
        &self,
        params: ExecParams,
    ) -> Result<(ExecResponse, watch::Sender<u64>, ExecProcessEventLog), JSONRPCErrorError> {
        let process_id = params.process_id.clone();
        let request_policy_decisions = params
            .network_proxy
            .as_ref()
            .is_some_and(|launch| launch.proxy.request_policy_decisions);
        if request_policy_decisions
            && (process_id.is_empty() || process_id.len() > MAX_NETWORK_POLICY_PROCESS_ID_BYTES)
        {
            return Err(invalid_params(format!(
                "callback-enabled process ID must be non-empty and at most {MAX_NETWORK_POLICY_PROCESS_ID_BYTES} bytes"
            )));
        }
        let network_policy_shutdown = request_policy_decisions.then(CancellationToken::new);
        let network_policy_decider = network_policy_shutdown.as_ref().map(|process_shutdown| {
            network_policy_decider(
                process_id.clone(),
                Arc::clone(&self.inner.requests),
                process_shutdown.clone(),
            )
        });
        let prepared = prepare_exec_request(
            &params,
            child_env(&params),
            self.runtime_paths.as_ref(),
            network_policy_decider,
        )
        .await?;
        if prepared.command.is_empty() {
            return Err(invalid_params("argv must not be empty".to_string()));
        }

        let start = Arc::new(ProcessStart);
        {
            let mut process_map = self.inner.processes.lock().await;
            if process_map.contains_key(&process_id) {
                return Err(invalid_request(format!(
                    "process {process_id} already exists"
                )));
            }
            process_map.insert(
                process_id.clone(),
                ProcessEntry::Starting(Arc::clone(&start)),
            );
        }

        let spawned_result = codex_sandboxing::spawn_process(codex_sandboxing::SpawnRequest {
            command: &prepared.command,
            cwd: prepared.cwd.as_path(),
            env: &prepared.env,
            arg0: &prepared.arg0,
            sandbox: prepared.sandbox,
            windows_sandbox: prepared.windows_sandbox_spawn_request(),
            tty: params.tty,
            stdin_open: params.tty || params.pipe_stdin,
            inherited_fds: &[],
        })
        .await;
        let spawned = match spawned_result {
            Ok(spawned) => spawned,
            Err(err) => {
                let mut process_map = self.inner.processes.lock().await;
                if matches!(
                    process_map.get(&process_id),
                    Some(ProcessEntry::Starting(current)) if Arc::ptr_eq(current, &start)
                ) {
                    process_map.remove(&process_id);
                }
                return Err(internal_error(err.to_string()));
            }
        };

        let output_notify = Arc::new(Notify::new());
        let (wake_tx, _wake_rx) = watch::channel(0);
        let events = ExecProcessEventLog::new(
            PROCESS_EVENT_CHANNEL_CAPACITY,
            RETAINED_OUTPUT_BYTES_PER_PROCESS,
        );
        {
            let mut process_map = self.inner.processes.lock().await;
            if !matches!(
                process_map.get(&process_id),
                Some(ProcessEntry::Starting(current)) if Arc::ptr_eq(current, &start)
            ) {
                drop(process_map);
                spawned.session.terminate();
                return Err(invalid_request(format!(
                    "process {process_id} start was cancelled"
                )));
            }
            process_map.insert(
                process_id.clone(),
                ProcessEntry::Running(Box::new(RunningProcess {
                    session: spawned.session,
                    tty: params.tty,
                    pipe_stdin: params.pipe_stdin,
                    accepted_stdin_write_ids: Arc::new(
                        Mutex::new(AcceptedStdinWriteIds::default()),
                    ),
                    output: VecDeque::new(),
                    retained_bytes: 0,
                    next_seq: 1,
                    exit_code: None,
                    wake_tx: wake_tx.clone(),
                    events: events.clone(),
                    output_notify: Arc::clone(&output_notify),
                    open_streams: 2,
                    closed: false,
                    metrics: Some(self.inner.telemetry.process_started()),
                    termination_requested: false,
                    sandbox: prepared.sandbox,
                    sandbox_denied: false,
                    network_proxy_handle: prepared.network_proxy_handle,
                    network_policy_shutdown,
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

        Ok((ExecResponse { process_id }, wake_tx, events))
    }

    pub(crate) async fn exec(&self, params: ExecParams) -> Result<ExecResponse, JSONRPCErrorError> {
        self.start_process(params)
            .await
            .map(|(response, _, _)| response)
    }

    pub(crate) async fn exec_read(
        &self,
        params: ReadParams,
    ) -> Result<ReadResponse, JSONRPCErrorError> {
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
                if params.max_bytes.is_none() {
                    next_seq = process.next_seq;
                }
                (
                    ReadResponse {
                        chunks,
                        next_seq,
                        exited: process.exit_code.is_some(),
                        exit_code: process.exit_code,
                        closed: process.closed,
                        failure: None,
                        sandbox_denied: process.sandbox_denied,
                    },
                    Arc::clone(&process.output_notify),
                )
            };

            let has_new_terminal_event =
                response.exited && after_seq < response.next_seq.saturating_sub(1);
            if !response.chunks.is_empty()
                || response.closed
                || has_new_terminal_event
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
        let _input_bytes = params.chunk.0.len();
        if params.write_id.is_empty() {
            return Err(invalid_params("writeId must not be empty".to_string()));
        }

        let (writer_tx, accepted_stdin_write_ids) = {
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
            if !process.tty && !process.pipe_stdin {
                return Ok(WriteResponse {
                    status: WriteStatus::StdinClosed,
                });
            }
            (
                process.session.writer_sender(),
                Arc::clone(&process.accepted_stdin_write_ids),
            )
        };

        if accepted_stdin_write_ids
            .lock()
            .await
            .contains(&params.write_id)
        {
            return Ok(WriteResponse {
                status: WriteStatus::Accepted,
            });
        }

        let permit = writer_tx
            .reserve()
            .await
            .map_err(|_| internal_error("failed to write to process stdin".to_string()))?;
        let mut accepted_stdin_write_ids = accepted_stdin_write_ids.lock().await;
        if accepted_stdin_write_ids.contains(&params.write_id) {
            return Ok(WriteResponse {
                status: WriteStatus::Accepted,
            });
        }

        // After this synchronous send, record the write id before any further await.
        // Otherwise a cancelled RPC handler could retry and write the same bytes again.
        permit.send(params.chunk.into_inner());
        accepted_stdin_write_ids.remember(params.write_id);

        Ok(WriteResponse {
            status: WriteStatus::Accepted,
        })
    }

    pub(crate) async fn signal_process(
        &self,
        params: SignalParams,
    ) -> Result<SignalResponse, JSONRPCErrorError> {
        {
            let process_map = self.inner.processes.lock().await;
            match process_map.get(&params.process_id) {
                Some(ProcessEntry::Running(process)) => {
                    if process.exit_code.is_some() {
                        return Ok(SignalResponse {});
                    }
                    process
                        .session
                        .signal(pty_process_signal(params.signal))
                        .map_err(|err| internal_error(format!("failed to signal process: {err}")))?
                }
                Some(ProcessEntry::Starting(_)) | None => {}
            }
        }

        Ok(SignalResponse {})
    }

    pub(crate) async fn terminate_process(
        &self,
        params: TerminateParams,
    ) -> Result<TerminateResponse, JSONRPCErrorError> {
        let running = {
            let mut process_map = self.inner.processes.lock().await;
            match process_map.get_mut(&params.process_id) {
                Some(ProcessEntry::Running(process)) => {
                    if let Some(network_policy_shutdown) = &process.network_policy_shutdown {
                        network_policy_shutdown.cancel();
                    }
                    if process.exit_code.is_some() {
                        return Ok(TerminateResponse { running: false });
                    }
                    process.termination_requested = true;
                    process.session.terminate();
                    true
                }
                Some(ProcessEntry::Starting(_)) => {
                    process_map.remove(&params.process_id);
                    true
                }
                None => false,
            }
        };

        Ok(TerminateResponse { running })
    }
}

fn child_env(params: &ExecParams) -> HashMap<String, String> {
    let Some(env_policy) = &params.env_policy else {
        return params.env.clone();
    };

    let policy = shell_environment_policy(env_policy);
    let mut env = shell_environment::create_env(&policy, /*thread_id*/ None);
    env.extend(params.env.clone());
    env
}

fn shell_environment_policy(env_policy: &ExecEnvPolicy) -> ShellEnvironmentPolicy {
    ShellEnvironmentPolicy {
        inherit: env_policy.inherit.clone(),
        ignore_default_excludes: env_policy.ignore_default_excludes,
        exclude: env_policy
            .exclude
            .iter()
            .map(|pattern| EnvironmentVariablePattern::new_case_insensitive(pattern))
            .collect(),
        r#set: env_policy.r#set.clone(),
        include_only: env_policy
            .include_only
            .iter()
            .map(|pattern| EnvironmentVariablePattern::new_case_insensitive(pattern))
            .collect(),
        use_profile: false,
    }
}

impl LocalProcess {
    async fn start(&self, params: ExecParams) -> Result<StartedExecProcess, ExecServerError> {
        let (response, wake_tx, events) = self
            .start_process(params)
            .await
            .map_err(map_handler_error)?;
        Ok(StartedExecProcess {
            process: Arc::new(LocalExecProcess {
                process_id: response.process_id,
                backend: self.clone(),
                wake_tx,
                events,
            }),
        })
    }
}

impl ExecBackend for LocalProcess {
    fn start(&self, params: ExecParams) -> ExecBackendFuture<'_> {
        Box::pin(LocalProcess::start(self, params))
    }
}

impl LocalExecProcess {
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

    async fn signal(&self, signal: ProcessSignal) -> Result<(), ExecServerError> {
        self.backend.signal(&self.process_id, signal).await
    }

    async fn terminate(&self) -> Result<(), ExecServerError> {
        self.backend.terminate(&self.process_id).await
    }
}

impl ExecProcess for LocalExecProcess {
    fn process_id(&self) -> &ProcessId {
        &self.process_id
    }

    fn subscribe_wake(&self) -> watch::Receiver<u64> {
        self.wake_tx.subscribe()
    }

    fn subscribe_events(&self) -> ExecProcessEventReceiver {
        self.events.subscribe()
    }

    fn read(
        &self,
        after_seq: Option<u64>,
        max_bytes: Option<usize>,
        wait_ms: Option<u64>,
    ) -> ExecProcessFuture<'_, ReadResponse> {
        Box::pin(LocalExecProcess::read(self, after_seq, max_bytes, wait_ms))
    }

    fn write(&self, chunk: Vec<u8>) -> ExecProcessFuture<'_, WriteResponse> {
        Box::pin(LocalExecProcess::write(self, chunk))
    }

    fn signal(&self, signal: ProcessSignal) -> ExecProcessFuture<'_, ()> {
        Box::pin(LocalExecProcess::signal(self, signal))
    }

    fn terminate(&self) -> ExecProcessFuture<'_, ()> {
        Box::pin(LocalExecProcess::terminate(self))
    }
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
            write_id: format!(
                "local-{}",
                NEXT_LOCAL_STDIN_WRITE_ID.fetch_add(1, Ordering::Relaxed)
            ),
        })
        .await
        .map_err(map_handler_error)
    }

    async fn signal(
        &self,
        process_id: &ProcessId,
        signal: ProcessSignal,
    ) -> Result<(), ExecServerError> {
        self.signal_process(SignalParams {
            process_id: process_id.clone(),
            signal,
        })
        .await
        .map_err(map_handler_error)?;
        Ok(())
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

fn pty_process_signal(signal: ProcessSignal) -> PtyProcessSignal {
    match signal {
        ProcessSignal::Interrupt => PtyProcessSignal::Interrupt,
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
            while process.retained_bytes > RETAINED_OUTPUT_BYTES_PER_PROCESS
                || process.output.len() > RETAINED_OUTPUT_CHUNKS_PER_PROCESS
            {
                let Some(evicted) = process.output.pop_front() else {
                    break;
                };
                process.retained_bytes = process.retained_bytes.saturating_sub(evicted.chunk.len());
            }
            let _ = process.wake_tx.send(seq);
            let output = ProcessOutputChunk {
                seq,
                stream,
                chunk: chunk.into(),
            };
            process
                .events
                .publish(ExecProcessEvent::Output(output.clone()));
            ExecOutputDeltaNotification {
                process_id: process_id.clone(),
                seq,
                stream,
                chunk: output.chunk,
            }
        };
        output_notify.notify_waiters();
        if let Some(notifications) = notification_sender(&inner) {
            let _ = notifications
                .notify(crate::protocol::EXEC_OUTPUT_DELTA_METHOD, &notification)
                .await;
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
    let sandboxed = {
        let mut processes = inner.processes.lock().await;
        match processes.get_mut(&process_id) {
            Some(ProcessEntry::Running(process)) => {
                let sandboxed = process.sandbox != SandboxType::None;
                if let Some(metrics) = process.metrics.take() {
                    metrics.finish(if process.termination_requested {
                        "terminated"
                    } else if exit_code == 0 {
                        "success"
                    } else {
                        "error"
                    });
                }
                sandboxed
            }
            Some(ProcessEntry::Starting(_)) | None => false,
        }
    };
    if sandboxed {
        let _ = tokio::time::timeout(Duration::from_millis(20), output_notify.notified()).await;
    }
    let notification = {
        let mut processes = inner.processes.lock().await;
        if let Some(ProcessEntry::Running(process)) = processes.get_mut(&process_id) {
            let seq = process.next_seq;
            process.next_seq += 1;
            process.exit_code = Some(exit_code);
            if process.sandbox != SandboxType::None {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                let mut aggregated = Vec::new();
                for chunk in &process.output {
                    match chunk.stream {
                        ExecOutputStream::Stdout | ExecOutputStream::Pty => {
                            stdout.extend_from_slice(&chunk.chunk);
                        }
                        ExecOutputStream::Stderr => stderr.extend_from_slice(&chunk.chunk),
                    }
                    aggregated.extend_from_slice(&chunk.chunk);
                }
                let exec_output = ExecToolCallOutput {
                    exit_code,
                    stdout: StreamOutput::new(String::from_utf8_lossy(&stdout).into_owned()),
                    stderr: StreamOutput::new(String::from_utf8_lossy(&stderr).into_owned()),
                    aggregated_output: StreamOutput::new(
                        String::from_utf8_lossy(&aggregated).into_owned(),
                    ),
                    ..Default::default()
                };
                process.sandbox_denied = is_likely_sandbox_denied(process.sandbox, &exec_output);
            }
            let _ = process.wake_tx.send(seq);
            process.events.publish(ExecProcessEvent::Exited {
                seq,
                exit_code,
                sandbox_denied: Some(process.sandbox_denied),
            });
            Some(ExecExitedNotification {
                process_id: process_id.clone(),
                seq,
                exit_code,
                sandbox_denied: Some(process.sandbox_denied),
            })
        } else {
            None
        }
    };
    output_notify.notify_waiters();
    if let Some(notification) = notification
        && let Some(notifications) = notification_sender(&inner)
    {
        let _ = notifications
            .notify(crate::protocol::EXEC_EXITED_METHOD, &notification)
            .await;
    }

    maybe_emit_closed(process_id, Arc::clone(&inner)).await;
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
    let (notification, output_notify, network_proxy_handle) = {
        let mut processes = inner.processes.lock().await;
        let Some(ProcessEntry::Running(process)) = processes.get_mut(&process_id) else {
            return;
        };

        if process.closed || process.open_streams != 0 || process.exit_code.is_none() {
            return;
        }

        process.closed = true;
        if let Some(network_policy_shutdown) = process.network_policy_shutdown.take() {
            network_policy_shutdown.cancel();
        }
        let seq = process.next_seq;
        process.next_seq += 1;
        let _ = process.wake_tx.send(seq);
        process.events.publish(ExecProcessEvent::Closed { seq });
        (
            ExecClosedNotification {
                process_id: process_id.clone(),
                seq,
            },
            Arc::clone(&process.output_notify),
            process.network_proxy_handle.take(),
        )
    };

    if let Some(network_proxy_handle) = network_proxy_handle
        && let Err(err) = network_proxy_handle.shutdown().await
    {
        tracing::warn!("failed to shut down executor network proxy: {err}");
    }

    output_notify.notify_waiters();
    let cleanup_process_id = process_id.clone();
    let cleanup_inner = Arc::clone(&inner);
    tokio::spawn(async move {
        tokio::time::sleep(EXITED_PROCESS_RETENTION).await;
        let mut processes = cleanup_inner.processes.lock().await;
        match processes.entry(cleanup_process_id) {
            Entry::Occupied(entry) => {
                if matches!(entry.get(), ProcessEntry::Running(process) if process.closed) {
                    entry.remove();
                }
            }
            Entry::Vacant(_) => {}
        }
    });

    if let Some(notifications) = notification_sender(&inner) {
        let _ = notifications
            .notify(EXEC_CLOSED_METHOD, &notification)
            .await;
    }
}

fn notification_sender(inner: &Inner) -> Option<RpcNotificationSender> {
    inner
        .notifications
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_exec_server_protocol::JSONRPCMessage;
    use codex_exec_server_protocol::JSONRPCResponse;
    use codex_exec_server_protocol::RequestId;
    use codex_network_proxy::NetworkProxy;
    use codex_network_proxy::NetworkProxyConfig;
    use codex_network_proxy::NetworkProxyState;
    use codex_network_proxy::RemoteNetworkProxyConfig;
    use codex_network_proxy::RemoteNetworkProxyLaunchConfig;
    use codex_otel::MetricsConfig;
    use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
    use codex_utils_path_uri::PathUri;
    use codex_utils_pty::ProcessDriver;
    use opentelemetry_sdk::metrics::InMemoryMetricExporter;
    use opentelemetry_sdk::metrics::data::AggregatedMetrics;
    use opentelemetry_sdk::metrics::data::MetricData;
    use pretty_assertions::assert_eq;
    #[cfg(not(target_os = "windows"))]
    use tokio::io::AsyncReadExt;
    #[cfg(not(target_os = "windows"))]
    use tokio::io::AsyncWriteExt;
    use tokio::sync::oneshot;
    use tokio::time::timeout;

    #[cfg(not(target_os = "windows"))]
    use crate::protocol::ExecServerNetworkPolicyDecision;
    #[cfg(not(target_os = "windows"))]
    use crate::protocol::NETWORK_POLICY_REQUEST_METHOD;
    #[cfg(not(target_os = "windows"))]
    use crate::protocol::NetworkPolicyRequestParams;
    #[cfg(not(target_os = "windows"))]
    use crate::protocol::NetworkPolicyRequestResponse;

    fn test_exec_params(env: HashMap<String, String>) -> ExecParams {
        ExecParams {
            process_id: ProcessId::from("env-test"),
            argv: vec!["true".to_string()],
            cwd: PathUri::from_host_native_path(std::env::current_dir().expect("cwd"))
                .expect("cwd URI"),
            env_policy: None,
            env,
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: None,
            enforce_managed_network: false,
            managed_network: None,
            network_proxy: None,
        }
    }

    fn telemetry_backend() -> (
        LocalProcess,
        codex_otel::MetricsClient,
        InMemoryMetricExporter,
    ) {
        let exporter = InMemoryMetricExporter::default();
        let metrics = codex_otel::MetricsClient::new(MetricsConfig::in_memory(
            "test",
            "codex-exec-server",
            env!("CARGO_PKG_VERSION"),
            exporter.clone(),
        ))
        .expect("metrics");
        let telemetry = ExecServerTelemetry::new(metrics.clone());
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(NOTIFICATION_CHANNEL_CAPACITY);
        tokio::spawn(async move { while outgoing_rx.recv().await.is_some() {} });
        (
            LocalProcess::with_runtime_paths(
                RpcNotificationSender::new(outgoing_tx),
                telemetry,
                /*runtime_paths*/ None,
            ),
            metrics,
            exporter,
        )
    }

    fn assert_finished_process_result(
        metrics: codex_otel::MetricsClient,
        exporter: &InMemoryMetricExporter,
        expected: &str,
    ) {
        metrics.shutdown().expect("shutdown metrics");
        let resource_metrics = exporter
            .get_finished_metrics()
            .expect("finished metrics")
            .into_iter()
            .last()
            .expect("metrics export");
        let finished_processes = resource_metrics
            .scope_metrics()
            .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
            .find(|metric| metric.name() == "exec_server_processes_finished_total")
            .expect("finished process metric");
        let AggregatedMetrics::U64(MetricData::Sum(sum)) = finished_processes.data() else {
            panic!("finished process metric should be a u64 sum");
        };
        let results = sum
            .data_points()
            .flat_map(opentelemetry_sdk::metrics::data::SumDataPoint::attributes)
            .filter(|attribute| attribute.key.as_str() == "result")
            .map(|attribute| attribute.value.as_str().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(results, vec![expected.to_string()]);
    }

    #[tokio::test]
    async fn start_process_rejects_non_native_cwd_before_launch() {
        #[cfg(unix)]
        let uri = "file://server/share/checkout";
        #[cfg(windows)]
        let uri = "file:///usr/local/checkout";
        let cwd = PathUri::parse(uri).expect("non-native cwd URI");
        let source = cwd
            .to_abs_path()
            .expect_err("cwd should not be native to this host");
        let expected = invalid_params(format!(
            "cwd URI `{cwd}` is not valid on this exec-server host: {source}"
        ));
        let mut params = test_exec_params(HashMap::new());
        params.cwd = cwd;

        let result = LocalProcess::default().start_process(params).await;
        let Err(error) = result else {
            panic!("non-native cwd should be rejected");
        };

        assert_eq!(error, expected);
    }

    #[tokio::test]
    async fn callback_enabled_start_bounds_process_id_before_proxy_launch() {
        let mut proxy_config =
            RemoteNetworkProxyConfig::from_effective_config(&NetworkProxyConfig::default())
                .expect("remote proxy config");
        proxy_config.request_policy_decisions = true;
        let proxy = RemoteNetworkProxyLaunchConfig::new(proxy_config);
        let expected = invalid_params(format!(
            "callback-enabled process ID must be non-empty and at most {MAX_NETWORK_POLICY_PROCESS_ID_BYTES} bytes"
        ));

        for process_id in [
            String::new(),
            "p".repeat(MAX_NETWORK_POLICY_PROCESS_ID_BYTES + 1),
        ] {
            let mut params = test_exec_params(HashMap::new());
            params.process_id = ProcessId::from(process_id);
            params.network_proxy = Some(proxy.clone());
            let error = LocalProcess::default()
                .start_process(params)
                .await
                .err()
                .expect("invalid callback process ID should be rejected");

            assert_eq!(error, expected);
        }

        let mut boundary = test_exec_params(HashMap::new());
        boundary.process_id = ProcessId::from("p".repeat(MAX_NETWORK_POLICY_PROCESS_ID_BYTES));
        boundary.network_proxy = Some(proxy);
        boundary.argv.clear();
        let error = LocalProcess::default()
            .start_process(boundary)
            .await
            .err()
            .expect("valid boundary process ID should proceed to process preparation");
        #[cfg(not(target_os = "windows"))]
        assert!(
            error
                .message
                .contains("executor-local network proxy launch requires an enabled proxy")
        );
        #[cfg(target_os = "windows")]
        assert_eq!(error, invalid_params("argv must not be empty".to_string()));

        for process_id in [
            String::new(),
            "p".repeat(MAX_NETWORK_POLICY_PROCESS_ID_BYTES + 1),
        ] {
            let mut ordinary = test_exec_params(HashMap::new());
            ordinary.process_id = ProcessId::from(process_id);
            ordinary.argv.clear();
            let error = LocalProcess::default()
                .start_process(ordinary)
                .await
                .err()
                .expect("empty argv should be rejected after ID validation");

            assert_eq!(error, invalid_params("argv must not be empty".to_string()));
        }
    }

    #[test]
    fn child_env_defaults_to_exact_env() {
        let params = test_exec_params(HashMap::from([("ONLY_THIS".to_string(), "1".to_string())]));

        assert_eq!(
            child_env(&params),
            HashMap::from([("ONLY_THIS".to_string(), "1".to_string())])
        );
    }

    #[test]
    fn child_env_applies_policy_then_overlay() {
        let mut params = test_exec_params(HashMap::from([
            ("OVERLAY".to_string(), "overlay".to_string()),
            ("POLICY_SET".to_string(), "overlay-wins".to_string()),
        ]));
        params.env_policy = Some(ExecEnvPolicy {
            inherit: ShellEnvironmentPolicyInherit::None,
            ignore_default_excludes: true,
            exclude: Vec::new(),
            r#set: HashMap::from([("POLICY_SET".to_string(), "policy".to_string())]),
            include_only: Vec::new(),
        });

        let mut expected = HashMap::from([
            ("OVERLAY".to_string(), "overlay".to_string()),
            ("POLICY_SET".to_string(), "overlay-wins".to_string()),
        ]);
        if cfg!(target_os = "windows") {
            expected.insert("PATHEXT".to_string(), ".COM;.EXE;.BAT;.CMD".to_string());
        }

        assert_eq!(child_env(&params), expected);
    }

    #[tokio::test]
    async fn exit_before_shutdown_records_success() {
        let (backend, metrics, exporter) = telemetry_backend();
        let mut process = spawn_test_process(&backend, "exit-before-shutdown").await;

        process.exit(/*exit_code*/ 0);
        let _ = read_process_until_change(&backend, &process.process_id, /*after_seq*/ None).await;
        backend.shutdown().await;

        assert_finished_process_result(metrics, &exporter, "success");
    }

    #[tokio::test]
    async fn termination_request_before_exit_records_terminated() {
        let (backend, metrics, exporter) = telemetry_backend();
        let mut process = spawn_test_process(&backend, "terminate-before-exit").await;

        assert_eq!(
            backend
                .terminate_process(TerminateParams {
                    process_id: process.process_id.clone(),
                })
                .await
                .expect("terminate process"),
            TerminateResponse { running: true },
        );
        process.exit(/*exit_code*/ 0);
        let _ = read_process_until_change(&backend, &process.process_id, /*after_seq*/ None).await;
        backend.shutdown().await;

        assert_finished_process_result(metrics, &exporter, "terminated");
    }

    #[tokio::test]
    async fn termination_request_after_exit_cancels_network_policy_decisions() {
        let backend = LocalProcess::default();
        let mut process = spawn_test_process(&backend, "terminate-after-exit").await;
        let network_policy_shutdown = CancellationToken::new();
        {
            let mut processes = backend.inner.processes.lock().await;
            let Some(ProcessEntry::Running(running)) = processes.get_mut(&process.process_id)
            else {
                panic!("test process should be running");
            };
            running.network_policy_shutdown = Some(network_policy_shutdown.clone());
        }

        process.exit(/*exit_code*/ 0);
        let response =
            read_process_until_change(&backend, &process.process_id, /*after_seq*/ None).await;
        assert!(response.exited);
        assert!(!response.closed);
        assert!(!network_policy_shutdown.is_cancelled());
        assert_eq!(
            backend
                .terminate_process(TerminateParams {
                    process_id: process.process_id.clone(),
                })
                .await
                .expect("terminate exited process"),
            TerminateResponse { running: false },
        );
        assert!(network_policy_shutdown.is_cancelled());

        drop(process.stdout_tx);
        drop(process.stderr_tx);
        let _ = read_process_until_closed(&backend, &process.process_id).await;
        backend.shutdown().await;
    }

    #[tokio::test]
    async fn shutdown_before_exit_records_terminated() {
        let (backend, metrics, exporter) = telemetry_backend();
        let mut process = spawn_test_process(&backend, "shutdown-before-exit").await;

        backend.shutdown().await;
        process.exit(/*exit_code*/ 0);

        assert_finished_process_result(metrics, &exporter, "terminated");
    }

    #[tokio::test]
    async fn exited_process_retains_late_output_past_retention() {
        let backend = LocalProcess::default();
        let mut process = spawn_test_process(&backend, "proc-late-output").await;

        process.exit(/*exit_code*/ 0);
        let exit_response =
            read_process_until_change(&backend, &process.process_id, /*after_seq*/ None).await;
        assert_eq!(
            exit_response,
            ReadResponse {
                chunks: Vec::new(),
                next_seq: 2,
                exited: true,
                exit_code: Some(0),
                closed: false,
                failure: None,
                sandbox_denied: false,
            }
        );

        tokio::time::sleep(EXITED_PROCESS_RETENTION + Duration::from_millis(10)).await;
        process
            .stdout_tx
            .send(b"late output after retention\n".to_vec())
            .await
            .expect("send late stdout");

        let late_response =
            read_process_until_change(&backend, &process.process_id, /*after_seq*/ Some(1)).await;
        assert_eq!(
            late_response.chunks,
            vec![ProcessOutputChunk {
                seq: 2,
                stream: ExecOutputStream::Stdout,
                chunk: b"late output after retention\n".to_vec().into(),
            }]
        );
        assert_eq!(late_response.exit_code, Some(0));
        assert!(!late_response.closed);

        drop(process.stdout_tx);
        drop(process.stderr_tx);
        let _closed_response = timeout(
            Duration::from_secs(1),
            read_process_until_closed(&backend, &process.process_id),
        )
        .await
        .expect("process should close");
        let replay_after_exit = backend
            .exec_read(ReadParams {
                process_id: process.process_id.clone(),
                after_seq: Some(1),
                max_bytes: None,
                wait_ms: Some(0),
            })
            .await
            .expect("closed process should remain readable");
        assert_eq!(replay_after_exit.next_seq, 4);
        backend.shutdown().await;
    }

    #[tokio::test]
    async fn process_read_replay_is_bounded_by_chunk_count() {
        let backend = LocalProcess::default();
        let process = spawn_test_process(&backend, "proc-chunk-count").await;
        let retained_chunk_count = RETAINED_OUTPUT_CHUNKS_PER_PROCESS as u64;

        {
            let mut processes = backend.inner.processes.lock().await;
            let Some(ProcessEntry::Running(running)) = processes.get_mut(&process.process_id)
            else {
                panic!("process should be running");
            };
            running.output = (1..=retained_chunk_count)
                .map(|seq| RetainedOutputChunk {
                    seq,
                    stream: ExecOutputStream::Stdout,
                    chunk: vec![b'x'],
                })
                .collect();
            running.retained_bytes = RETAINED_OUTPUT_CHUNKS_PER_PROCESS;
            running.next_seq = retained_chunk_count + 1;
        }

        process
            .stdout_tx
            .send(vec![b'y'])
            .await
            .expect("send output beyond retained chunk limit");
        timeout(Duration::from_secs(1), async {
            loop {
                let output_recorded = {
                    let processes = backend.inner.processes.lock().await;
                    let Some(ProcessEntry::Running(running)) = processes.get(&process.process_id)
                    else {
                        panic!("process should be running");
                    };
                    running.next_seq == retained_chunk_count + 2
                };
                if output_recorded {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("output should be retained");

        let response = backend
            .exec_read(ReadParams {
                process_id: process.process_id.clone(),
                after_seq: None,
                max_bytes: None,
                wait_ms: Some(0),
            })
            .await
            .expect("read retained output");
        let mut expected_chunks = (2..=retained_chunk_count)
            .map(|seq| ProcessOutputChunk {
                seq,
                stream: ExecOutputStream::Stdout,
                chunk: vec![b'x'].into(),
            })
            .collect::<Vec<_>>();
        expected_chunks.push(ProcessOutputChunk {
            seq: retained_chunk_count + 1,
            stream: ExecOutputStream::Stdout,
            chunk: vec![b'y'].into(),
        });
        assert_eq!(
            response,
            ReadResponse {
                chunks: expected_chunks,
                next_seq: retained_chunk_count + 2,
                exited: false,
                exit_code: None,
                closed: false,
                failure: None,
                sandbox_denied: false,
            }
        );

        let message = JSONRPCMessage::Response(JSONRPCResponse {
            id: RequestId::Integer(1),
            result: serde_json::to_value(response).expect("serialize process/read response"),
        });
        let encoded = serde_json::to_string(&message).expect("encode JSON-RPC response");
        let decoded = serde_json::from_str::<JSONRPCMessage>(&encoded)
            .expect("retained process/read response should fit the JSON value budget");
        assert_eq!(decoded, message);

        backend.shutdown().await;
    }

    #[tokio::test]
    async fn exited_process_keeps_network_proxy_until_inherited_streams_close() {
        let backend = LocalProcess::default();
        let mut process = spawn_test_process(&backend, "proc-background-child").await;
        let (outgoing_tx, outgoing_rx) = mpsc::channel(NOTIFICATION_CHANNEL_CAPACITY);
        #[cfg(not(target_os = "windows"))]
        let mut outgoing_rx = outgoing_rx;
        #[cfg(target_os = "windows")]
        let _outgoing_rx = outgoing_rx;
        let requests = RpcNotificationSender::new(outgoing_tx).request_sender();
        *backend
            .inner
            .requests
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(requests.clone());
        let network_policy_shutdown = CancellationToken::new();
        let decider = network_policy_decider(
            process.process_id.clone(),
            Arc::clone(&backend.inner.requests),
            network_policy_shutdown.clone(),
        );
        let config = NetworkProxyConfig {
            enabled: true,
            ..Default::default()
        };
        let proxy_config = RemoteNetworkProxyConfig::from_effective_config(&config)
            .expect("build remote network proxy config");
        let state = NetworkProxyState::from_remote_launch_config(
            RemoteNetworkProxyLaunchConfig::new(proxy_config),
        )
        .expect("build network proxy state");
        let proxy = NetworkProxy::builder()
            .state(Arc::new(state))
            .policy_decider_arc(decider)
            .build()
            .await
            .expect("build network proxy");
        let handle = proxy.run().await.expect("start network proxy");
        let prepared = proxy
            .prepare_for_optional_environment(HashMap::new(), /*environment_id*/ None)
            .expect("prepare network proxy environment");
        let proxy_addr: std::net::SocketAddr = prepared
            .env
            .get("HTTP_PROXY")
            .and_then(|value| value.strip_prefix("http://"))
            .expect("HTTP proxy address")
            .parse()
            .expect("parse HTTP proxy address");

        {
            let mut processes = backend.inner.processes.lock().await;
            let Some(ProcessEntry::Running(running)) = processes.get_mut(&process.process_id)
            else {
                panic!("test process should be running");
            };
            running.network_proxy_handle = Some(handle);
            running.network_policy_shutdown = Some(network_policy_shutdown.clone());
        }

        process.exit(/*exit_code*/ 0);
        let exit_response =
            read_process_until_change(&backend, &process.process_id, /*after_seq*/ None).await;
        assert!(exit_response.exited);
        assert!(!exit_response.closed);
        assert!(!network_policy_shutdown.is_cancelled());
        let stream = tokio::net::TcpStream::connect(proxy_addr)
            .await
            .expect("proxy should remain available to a child holding inherited output streams");
        #[cfg(target_os = "windows")]
        {
            assert!(proxy.network_proxy_restricting_sid(None).is_some());
            drop(stream);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let mut stream = stream;
            stream
                .write_all(b"CONNECT 8.8.8.8:443 HTTP/1.1\r\nHost: 8.8.8.8:443\r\n\r\n")
                .await
                .expect("write CONNECT request");
            let outbound = timeout(Duration::from_secs(1), outgoing_rx.recv())
                .await
                .expect("policy request should arrive")
                .expect("policy request");
            let RpcServerOutboundMessage::Request(request) = outbound else {
                panic!("expected policy request");
            };
            assert_eq!(request.method, NETWORK_POLICY_REQUEST_METHOD);
            let params: NetworkPolicyRequestParams =
                serde_json::from_value(request.params.expect("request params"))
                    .expect("deserialize policy request");
            assert_eq!(params.process_id, process.process_id);
            assert_eq!(params.request.host, "8.8.8.8");
            requests.complete(
                request.id,
                Ok(serde_json::to_value(NetworkPolicyRequestResponse {
                    decision: ExecServerNetworkPolicyDecision::Deny {
                        reason: "not_allowed".to_string(),
                    },
                })
                .expect("serialize policy response")),
            );
            let mut response = [0_u8; 256];
            let response_len = timeout(Duration::from_secs(1), stream.read(&mut response))
                .await
                .expect("proxy response timeout")
                .expect("read proxy response");
            assert!(String::from_utf8_lossy(&response[..response_len]).starts_with("HTTP/1.1 403"));
        }

        drop(process.stdout_tx);
        drop(process.stderr_tx);
        let closed_response = timeout(
            Duration::from_secs(1),
            read_process_until_closed(&backend, &process.process_id),
        )
        .await
        .expect("process should close");
        assert!(closed_response.closed);
        assert!(network_policy_shutdown.is_cancelled());
        #[cfg(target_os = "windows")]
        assert_eq!(proxy.network_proxy_restricting_sid(None), None);
        #[cfg(not(target_os = "windows"))]
        assert!(tokio::net::TcpStream::connect(proxy_addr).await.is_err());
        backend.shutdown().await;
    }

    #[tokio::test]
    async fn closed_process_is_evicted_after_retention() {
        let backend = LocalProcess::default();
        let mut process = spawn_test_process(&backend, "proc-closed-eviction").await;
        let process_id = process.process_id.clone();

        process.exit(/*exit_code*/ 0);
        drop(process.stdout_tx);
        drop(process.stderr_tx);

        let closed_response = timeout(
            Duration::from_secs(1),
            read_process_until_closed(&backend, &process_id),
        )
        .await
        .expect("process should close");
        assert!(closed_response.closed);

        timeout(Duration::from_secs(1), async {
            loop {
                {
                    let processes = backend.inner.processes.lock().await;
                    if !processes.contains_key(&process_id) {
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("closed process should be evicted");
        backend.shutdown().await;
    }

    struct TestProcess {
        process_id: ProcessId,
        stdout_tx: mpsc::Sender<Vec<u8>>,
        stderr_tx: mpsc::Sender<Vec<u8>>,
        exit_tx: Option<oneshot::Sender<i32>>,
    }

    impl TestProcess {
        fn exit(&mut self, exit_code: i32) {
            self.exit_tx
                .take()
                .expect("process should not have exited")
                .send(exit_code)
                .expect("send process exit");
        }
    }

    async fn spawn_test_process(backend: &LocalProcess, process_id: &str) -> TestProcess {
        let process_id = ProcessId::from(process_id);
        let (stdout_tx, stdout_rx) = mpsc::channel(16);
        let (stderr_tx, stderr_rx) = mpsc::channel(16);
        let (exit_tx, exit_rx) = oneshot::channel();
        let output_notify = Arc::new(Notify::new());
        let (wake_tx, _wake_rx) = watch::channel(0);
        let events = ExecProcessEventLog::new(
            PROCESS_EVENT_CHANNEL_CAPACITY,
            RETAINED_OUTPUT_BYTES_PER_PROCESS,
        );

        let mut processes = backend.inner.processes.lock().await;
        let previous = processes.insert(
            process_id.clone(),
            ProcessEntry::Running(Box::new(RunningProcess {
                session: dummy_session(),
                tty: false,
                pipe_stdin: false,
                accepted_stdin_write_ids: Arc::new(Mutex::new(AcceptedStdinWriteIds::default())),
                output: VecDeque::new(),
                retained_bytes: 0,
                next_seq: 1,
                exit_code: None,
                wake_tx: wake_tx.clone(),
                events: events.clone(),
                output_notify: Arc::clone(&output_notify),
                open_streams: 2,
                closed: false,
                metrics: Some(backend.inner.telemetry.process_started()),
                termination_requested: false,
                sandbox: SandboxType::None,
                sandbox_denied: false,
                network_proxy_handle: None,
                network_policy_shutdown: None,
            })),
        );
        assert!(previous.is_none());
        drop(processes);

        tokio::spawn(stream_output(
            process_id.clone(),
            ExecOutputStream::Stdout,
            stdout_rx,
            Arc::clone(&backend.inner),
            Arc::clone(&output_notify),
        ));
        tokio::spawn(stream_output(
            process_id.clone(),
            ExecOutputStream::Stderr,
            stderr_rx,
            Arc::clone(&backend.inner),
            Arc::clone(&output_notify),
        ));
        tokio::spawn(watch_exit(
            process_id.clone(),
            exit_rx,
            Arc::clone(&backend.inner),
            output_notify,
        ));

        TestProcess {
            process_id,
            stdout_tx,
            stderr_tx,
            exit_tx: Some(exit_tx),
        }
    }

    fn dummy_session() -> ExecCommandSession {
        let (writer_tx, _writer_rx) = mpsc::channel(1);
        let (_stdout_tx, stdout_rx) = tokio::sync::broadcast::channel(1);
        let (_stderr_tx, stderr_rx) = tokio::sync::broadcast::channel(1);
        let (_exit_tx, exit_rx) = oneshot::channel();

        codex_utils_pty::spawn_from_driver(ProcessDriver {
            writer_tx,
            stdout_rx,
            stderr_rx: Some(stderr_rx),
            exit_rx,
            terminator: None,
            writer_handle: None,
            resizer: None,
        })
        .session
    }

    async fn read_process_until_change(
        backend: &LocalProcess,
        process_id: &ProcessId,
        after_seq: Option<u64>,
    ) -> ReadResponse {
        timeout(
            Duration::from_secs(1),
            backend.exec_read(ReadParams {
                process_id: process_id.clone(),
                after_seq,
                max_bytes: None,
                wait_ms: Some(1_000),
            }),
        )
        .await
        .expect("process read should finish")
        .expect("process read")
    }

    async fn read_process_until_closed(
        backend: &LocalProcess,
        process_id: &ProcessId,
    ) -> ReadResponse {
        let mut after_seq = None;
        loop {
            let response = read_process_until_change(backend, process_id, after_seq).await;
            if response.closed {
                return response;
            }
            for chunk in &response.chunks {
                after_seq = Some(chunk.seq);
            }
            after_seq = response.next_seq.checked_sub(1).or(after_seq);
        }
    }
}
