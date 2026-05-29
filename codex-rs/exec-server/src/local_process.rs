use std::collections::HashMap;
use std::collections::VecDeque;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_protocol::config_types::EnvironmentVariablePattern;
use codex_protocol::config_types::ShellEnvironmentPolicy;
use codex_protocol::shell_environment;
use codex_utils_pty::ExecCommandSession;
use codex_utils_pty::TerminalSize;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::sync::watch;

use crate::ExecBackend;
use crate::ExecProcess;
use crate::ExecProcessEvent;
use crate::ExecProcessEventReceiver;
use crate::ExecServerError;
use crate::ProcessId;
use crate::StartedExecProcess;
use crate::process::ExecProcessEventLog;
use crate::protocol::EXEC_CLOSED_METHOD;
use crate::protocol::ExecClosedNotification;
use crate::protocol::ExecEnvPolicy;
use crate::protocol::ExecExitedNotification;
use crate::protocol::ExecOutputDeltaNotification;
use crate::protocol::ExecOutputStream;
use crate::protocol::ExecParams;
use crate::protocol::ExecResponse;
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
const PROCESS_EVENT_CHANNEL_CAPACITY: usize = 256;
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
    output: VecDeque<RetainedOutputChunk>,
    retained_bytes: usize,
    next_seq: u64,
    exit_code: Option<i32>,
    wake_tx: watch::Sender<u64>,
    events: ExecProcessEventLog,
    output_notify: Arc<Notify>,
    open_streams: usize,
    closed: bool,
}

enum ProcessEntry {
    Starting,
    Running(Box<RunningProcess>),
}

struct Inner {
    notifications: std::sync::RwLock<Option<RpcNotificationSender>>,
    processes: Mutex<HashMap<ProcessId, ProcessEntry>>,
}

#[derive(Clone)]
pub(crate) struct LocalProcess {
    inner: Arc<Inner>,
}

struct LocalExecProcess {
    process_id: ProcessId,
    backend: LocalProcess,
    wake_tx: watch::Sender<u64>,
    events: ExecProcessEventLog,
}

impl Default for LocalProcess {
    fn default() -> Self {
        let (outgoing_tx, mut outgoing_rx) =
            mpsc::channel::<RpcServerOutboundMessage>(NOTIFICATION_CHANNEL_CAPACITY);
        tokio::spawn(async move { while outgoing_rx.recv().await.is_some() {} });
        Self::new(RpcNotificationSender::new(outgoing_tx))
    }
}

impl LocalProcess {
    pub(crate) fn new(notifications: RpcNotificationSender) -> Self {
        Self {
            inner: Arc::new(Inner {
                notifications: std::sync::RwLock::new(Some(notifications)),
                processes: Mutex::new(HashMap::new()),
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

    pub(crate) fn set_notification_sender(&self, notifications: Option<RpcNotificationSender>) {
        let mut notification_sender = self
            .inner
            .notifications
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *notification_sender = notifications;
    }

    async fn start_process(
        &self,
        params: ExecParams,
    ) -> Result<(ExecResponse, watch::Sender<u64>, ExecProcessEventLog), JSONRPCErrorError> {
        let process_id = params.process_id.clone();
        let (program, args) = params
            .argv
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

        let env = child_env(&params);
        let spawned_result = if params.tty {
            codex_utils_pty::spawn_pty_process(
                program,
                args,
                params.cwd.as_path(),
                &env,
                &params.arg0,
                TerminalSize::default(),
            )
            .await
        } else if params.pipe_stdin {
            codex_utils_pty::spawn_pipe_process(
                program,
                args,
                params.cwd.as_path(),
                &env,
                &params.arg0,
            )
            .await
        } else {
            codex_utils_pty::spawn_pipe_process_no_stdin(
                program,
                args,
                params.cwd.as_path(),
                &env,
                &params.arg0,
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
        let events = ExecProcessEventLog::new(
            PROCESS_EVENT_CHANNEL_CAPACITY,
            RETAINED_OUTPUT_BYTES_PER_PROCESS,
        );
        {
            let mut process_map = self.inner.processes.lock().await;
            process_map.insert(
                process_id.clone(),
                ProcessEntry::Running(Box::new(RunningProcess {
                    session: spawned.session,
                    tty: params.tty,
                    pipe_stdin: params.pipe_stdin,
                    output: VecDeque::new(),
                    retained_bytes: 0,
                    next_seq: 1,
                    exit_code: None,
                    wake_tx: wake_tx.clone(),
                    events: events.clone(),
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
            if !process.tty && !process.pipe_stdin {
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

#[async_trait]
impl ExecBackend for LocalProcess {
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

#[async_trait]
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
    let notification = {
        let mut processes = inner.processes.lock().await;
        if let Some(ProcessEntry::Running(process)) = processes.get_mut(&process_id) {
            let seq = process.next_seq;
            process.next_seq += 1;
            process.exit_code = Some(exit_code);
            let _ = process.wake_tx.send(seq);
            process
                .events
                .publish(ExecProcessEvent::Exited { seq, exit_code });
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
    let (notification, output_notify) = {
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
        process.events.publish(ExecProcessEvent::Closed { seq });
        (
            ExecClosedNotification {
                process_id: process_id.clone(),
                seq,
            },
            Arc::clone(&process.output_notify),
        )
    };

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
    use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
    use codex_utils_pty::ProcessDriver;
    use pretty_assertions::assert_eq;
    use tokio::sync::oneshot;
    use tokio::time::timeout;

    fn test_exec_params(env: HashMap<String, String>) -> ExecParams {
        ExecParams {
            process_id: ProcessId::from("env-test"),
            argv: vec!["true".to_string()],
            cwd: std::path::PathBuf::from("/tmp"),
            env_policy: None,
            env,
            tty: false,
            pipe_stdin: false,
            arg0: None,
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
                output: VecDeque::new(),
                retained_bytes: 0,
                next_seq: 1,
                exit_code: None,
                wake_tx: wake_tx.clone(),
                events: events.clone(),
                output_notify: Arc::clone(&output_notify),
                open_streams: 2,
                closed: false,
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
