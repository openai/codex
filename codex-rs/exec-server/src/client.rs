use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::debug;
use tracing::warn;

use crate::protocol::EXEC_EXITED_METHOD;
use crate::protocol::EXEC_METHOD;
use crate::protocol::EXEC_OUTPUT_DELTA_METHOD;
use crate::protocol::EXEC_TERMINATE_METHOD;
use crate::protocol::EXEC_WRITE_METHOD;
use crate::protocol::ExecExitedNotification;
use crate::protocol::ExecOutputDeltaNotification;
use crate::protocol::ExecParams;
use crate::protocol::ExecResponse;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::InitializeParams;
use crate::protocol::InitializeResponse;
use crate::protocol::TerminateParams;
use crate::protocol::TerminateResponse;
use crate::protocol::WriteParams;
use crate::protocol::WriteResponse;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecServerLaunchCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
}

pub struct ExecServerProcess {
    process_id: String,
    pid: Option<u32>,
    output_rx: broadcast::Receiver<Vec<u8>>,
    writer_tx: mpsc::Sender<Vec<u8>>,
    status: Arc<RemoteProcessStatus>,
    client: ExecServerClient,
}

impl std::fmt::Debug for ExecServerProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecServerProcess")
            .field("process_id", &self.process_id)
            .field("pid", &self.pid)
            .field("has_exited", &self.has_exited())
            .field("exit_code", &self.exit_code())
            .finish()
    }
}

impl ExecServerProcess {
    pub fn writer_sender(&self) -> mpsc::Sender<Vec<u8>> {
        self.writer_tx.clone()
    }

    pub fn output_receiver(&self) -> broadcast::Receiver<Vec<u8>> {
        self.output_rx.resubscribe()
    }

    pub fn has_exited(&self) -> bool {
        self.status.has_exited()
    }

    pub fn exit_code(&self) -> Option<i32> {
        self.status.exit_code()
    }

    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    pub fn terminate(&self) {
        self.status.mark_exited(None);
        let client = self.client.clone();
        let process_id = self.process_id.clone();
        tokio::spawn(async move {
            let _ = client.terminate_process(&process_id).await;
        });
    }
}

impl std::fmt::Debug for RemoteProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteProcessStatus")
            .field("exited", &self.has_exited())
            .field("exit_code", &self.exit_code())
            .finish()
    }
}

struct RemoteProcessStatus {
    exited: AtomicBool,
    exit_code: StdMutex<Option<i32>>,
}

impl RemoteProcessStatus {
    fn new() -> Self {
        Self {
            exited: AtomicBool::new(false),
            exit_code: StdMutex::new(None),
        }
    }

    fn has_exited(&self) -> bool {
        self.exited.load(Ordering::SeqCst)
    }

    fn exit_code(&self) -> Option<i32> {
        self.exit_code.lock().ok().and_then(|guard| *guard)
    }

    fn mark_exited(&self, exit_code: Option<i32>) {
        self.exited.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.exit_code.lock() {
            *guard = exit_code;
        }
    }
}

struct RegisteredProcess {
    output_tx: broadcast::Sender<Vec<u8>>,
    status: Arc<RemoteProcessStatus>,
}

struct Inner {
    child: StdMutex<Option<Child>>,
    write_tx: mpsc::UnboundedSender<JSONRPCMessage>,
    pending: Mutex<HashMap<RequestId, oneshot::Sender<Result<Value, JSONRPCErrorError>>>>,
    processes: Mutex<HashMap<String, RegisteredProcess>>,
    next_request_id: AtomicI64,
    reader_task: JoinHandle<()>,
    writer_task: JoinHandle<()>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.reader_task.abort();
        self.writer_task.abort();
        if let Ok(mut child_guard) = self.child.lock()
            && let Some(child) = child_guard.as_mut()
        {
            let _ = child.start_kill();
        }
    }
}

#[derive(Clone)]
pub struct ExecServerClient {
    inner: Arc<Inner>,
}

#[derive(Debug, thiserror::Error)]
pub enum ExecServerError {
    #[error("failed to spawn exec-server: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("exec-server transport closed")]
    Closed,
    #[error("failed to serialize or deserialize exec-server JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("exec-server protocol error: {0}")]
    Protocol(String),
    #[error("exec-server rejected request ({code}): {message}")]
    Server { code: i64, message: String },
}

impl ExecServerClient {
    pub async fn spawn(command: ExecServerLaunchCommand) -> Result<Self, ExecServerError> {
        let mut child = Command::new(&command.program);
        child.args(&command.args);
        child.stdin(Stdio::piped());
        child.stdout(Stdio::piped());
        child.stderr(Stdio::inherit());
        child.kill_on_drop(true);

        let mut child = child.spawn().map_err(ExecServerError::Spawn)?;
        let stdin = child.stdin.take().ok_or_else(|| {
            ExecServerError::Protocol("exec-server stdin was not captured".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            ExecServerError::Protocol("exec-server stdout was not captured".to_string())
        })?;

        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<JSONRPCMessage>();
        let writer_task = tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(message) = write_rx.recv().await {
                let encoded = match serde_json::to_vec(&message) {
                    Ok(encoded) => encoded,
                    Err(err) => {
                        warn!("failed to encode exec-server message: {err}");
                        break;
                    }
                };
                if stdin.write_all(&encoded).await.is_err() {
                    break;
                }
                if stdin.write_all(b"\n").await.is_err() {
                    break;
                }
                if stdin.flush().await.is_err() {
                    break;
                }
            }
        });

        let pending = Mutex::new(HashMap::<
            RequestId,
            oneshot::Sender<Result<Value, JSONRPCErrorError>>,
        >::new());
        let processes = Mutex::new(HashMap::<String, RegisteredProcess>::new());
        let inner = Arc::new_cyclic(move |weak| {
            let weak = weak.clone();
            let reader_task = tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                loop {
                    let Some(inner) = weak.upgrade() else {
                        break;
                    };
                    let next_line = lines.next_line().await;
                    match next_line {
                        Ok(Some(line)) => {
                            if line.trim().is_empty() {
                                continue;
                            }
                            match serde_json::from_str::<JSONRPCMessage>(&line) {
                                Ok(message) => {
                                    if let Err(err) = handle_server_message(&inner, message).await {
                                        warn!("failed to handle exec-server message: {err}");
                                        break;
                                    }
                                }
                                Err(err) => {
                                    warn!("failed to parse exec-server message: {err}");
                                    break;
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(err) => {
                            warn!("failed to read exec-server stdout: {err}");
                            break;
                        }
                    }
                }
                if let Some(inner) = weak.upgrade() {
                    handle_transport_shutdown(&inner).await;
                }
            });

            Inner {
                child: StdMutex::new(Some(child)),
                write_tx,
                pending,
                processes,
                next_request_id: AtomicI64::new(1),
                reader_task,
                writer_task,
            }
        });

        let client = Self { inner };
        client.initialize().await?;
        Ok(client)
    }

    pub async fn start_process(
        &self,
        params: ExecParams,
    ) -> Result<ExecServerProcess, ExecServerError> {
        let process_id = params.process_id.clone();
        let status = Arc::new(RemoteProcessStatus::new());
        let (output_tx, output_rx) = broadcast::channel(256);
        self.inner.processes.lock().await.insert(
            process_id.clone(),
            RegisteredProcess {
                output_tx,
                status: Arc::clone(&status),
            },
        );

        let (writer_tx, mut writer_rx) = mpsc::channel::<Vec<u8>>(128);
        let client = self.clone();
        let write_process_id = process_id.clone();
        tokio::spawn(async move {
            while let Some(chunk) = writer_rx.recv().await {
                let request = WriteParams {
                    process_id: write_process_id.clone(),
                    chunk: chunk.into(),
                };
                if client.write_process(request).await.is_err() {
                    break;
                }
            }
        });

        let response = match self.request::<_, ExecResponse>(EXEC_METHOD, &params).await {
            Ok(response) => response,
            Err(err) => {
                self.inner.processes.lock().await.remove(&process_id);
                return Err(err);
            }
        };

        if let Some(exit_code) = response.exit_code {
            status.mark_exited(Some(exit_code));
        }

        Ok(ExecServerProcess {
            process_id,
            pid: response.pid,
            output_rx,
            writer_tx,
            status,
            client: self.clone(),
        })
    }

    async fn initialize(&self) -> Result<(), ExecServerError> {
        let _: InitializeResponse = self
            .request(
                INITIALIZE_METHOD,
                &InitializeParams {
                    client_name: "codex-core".to_string(),
                },
            )
            .await?;
        self.notify(INITIALIZED_METHOD, &serde_json::json!({}))
            .await
    }

    async fn write_process(&self, params: WriteParams) -> Result<WriteResponse, ExecServerError> {
        self.request(EXEC_WRITE_METHOD, &params).await
    }

    async fn terminate_process(
        &self,
        process_id: &str,
    ) -> Result<TerminateResponse, ExecServerError> {
        self.request(
            EXEC_TERMINATE_METHOD,
            &TerminateParams {
                process_id: process_id.to_string(),
            },
        )
        .await
    }

    async fn notify<P: Serialize>(&self, method: &str, params: &P) -> Result<(), ExecServerError> {
        let params = serde_json::to_value(params)?;
        self.inner
            .write_tx
            .send(JSONRPCMessage::Notification(JSONRPCNotification {
                method: method.to_string(),
                params: Some(params),
            }))
            .map_err(|_| ExecServerError::Closed)
    }

    async fn request<P, R>(&self, method: &str, params: &P) -> Result<R, ExecServerError>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let request_id =
            RequestId::Integer(self.inner.next_request_id.fetch_add(1, Ordering::SeqCst));
        let (response_tx, response_rx) = oneshot::channel();
        self.inner
            .pending
            .lock()
            .await
            .insert(request_id.clone(), response_tx);

        let params = serde_json::to_value(params)?;
        let message = JSONRPCMessage::Request(JSONRPCRequest {
            id: request_id.clone(),
            method: method.to_string(),
            params: Some(params),
            trace: None,
        });

        if self.inner.write_tx.send(message).is_err() {
            self.inner.pending.lock().await.remove(&request_id);
            return Err(ExecServerError::Closed);
        }

        let result = response_rx.await.map_err(|_| ExecServerError::Closed)?;
        match result {
            Ok(value) => serde_json::from_value(value).map_err(ExecServerError::from),
            Err(error) => Err(ExecServerError::Server {
                code: error.code,
                message: error.message,
            }),
        }
    }
}

async fn handle_server_message(
    inner: &Arc<Inner>,
    message: JSONRPCMessage,
) -> Result<(), ExecServerError> {
    match message {
        JSONRPCMessage::Response(JSONRPCResponse { id, result }) => {
            if let Some(tx) = inner.pending.lock().await.remove(&id) {
                let _ = tx.send(Ok(result));
            }
        }
        JSONRPCMessage::Error(JSONRPCError { id, error }) => {
            if let Some(tx) = inner.pending.lock().await.remove(&id) {
                let _ = tx.send(Err(error));
            }
        }
        JSONRPCMessage::Notification(notification) => {
            handle_server_notification(inner, notification).await?;
        }
        JSONRPCMessage::Request(request) => {
            return Err(ExecServerError::Protocol(format!(
                "unexpected exec-server request from child: {}",
                request.method
            )));
        }
    }

    Ok(())
}

async fn handle_server_notification(
    inner: &Arc<Inner>,
    notification: JSONRPCNotification,
) -> Result<(), ExecServerError> {
    match notification.method.as_str() {
        EXEC_OUTPUT_DELTA_METHOD => {
            let params: ExecOutputDeltaNotification =
                serde_json::from_value(notification.params.unwrap_or(Value::Null))?;
            let chunk = params.chunk.into_inner();
            let processes = inner.processes.lock().await;
            if let Some(process) = processes.get(&params.process_id) {
                let _ = process.output_tx.send(chunk);
            }
        }
        EXEC_EXITED_METHOD => {
            let params: ExecExitedNotification =
                serde_json::from_value(notification.params.unwrap_or(Value::Null))?;
            let mut processes = inner.processes.lock().await;
            if let Some(process) = processes.remove(&params.process_id) {
                process.status.mark_exited(Some(params.exit_code));
            }
        }
        other => {
            debug!("ignoring unknown exec-server notification: {other}");
        }
    }
    Ok(())
}

async fn handle_transport_shutdown(inner: &Arc<Inner>) {
    let pending = {
        let mut pending = inner.pending.lock().await;
        pending.drain().map(|(_, tx)| tx).collect::<Vec<_>>()
    };
    for tx in pending {
        let _ = tx.send(Err(JSONRPCErrorError {
            code: -32000,
            data: None,
            message: "exec-server transport closed".to_string(),
        }));
    }

    let processes = {
        let mut processes = inner.processes.lock().await;
        processes
            .drain()
            .map(|(_, process)| process)
            .collect::<Vec<_>>()
    };
    for process in processes {
        process.status.mark_exited(None);
    }
}
