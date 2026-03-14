use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_utils_pty::ExecCommandSession;
use codex_utils_pty::TerminalSize;
use serde::Serialize;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::BufWriter;
use tokio::sync::Mutex;

use crate::protocol::EXEC_EXITED_METHOD;
use crate::protocol::EXEC_METHOD;
use crate::protocol::EXEC_OUTPUT_DELTA_METHOD;
use crate::protocol::EXEC_TERMINATE_METHOD;
use crate::protocol::EXEC_WRITE_METHOD;
use crate::protocol::ExecExitedNotification;
use crate::protocol::ExecOutputDeltaNotification;
use crate::protocol::ExecOutputStream;
use crate::protocol::ExecParams;
use crate::protocol::ExecResponse;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::InitializeResponse;
use crate::protocol::PROTOCOL_VERSION;
use crate::protocol::TerminateParams;
use crate::protocol::TerminateResponse;
use crate::protocol::WriteParams;
use crate::protocol::WriteResponse;

struct RunningProcess {
    session: ExecCommandSession,
    tty: bool,
    stdout_buffer: Arc<StdMutex<BoundedBytesBuffer>>,
    stderr_buffer: Arc<StdMutex<BoundedBytesBuffer>>,
}

#[derive(Debug)]
struct BoundedBytesBuffer {
    max_bytes: usize,
    bytes: VecDeque<u8>,
}

impl BoundedBytesBuffer {
    fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            bytes: VecDeque::with_capacity(max_bytes.min(8192)),
        }
    }

    fn push_chunk(&mut self, chunk: &[u8]) {
        if self.max_bytes == 0 {
            return;
        }
        for byte in chunk {
            self.bytes.push_back(*byte);
            if self.bytes.len() > self.max_bytes {
                self.bytes.pop_front();
            }
        }
    }

    fn snapshot(&self) -> Vec<u8> {
        self.bytes.iter().copied().collect()
    }
}

pub async fn run_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let writer = Arc::new(Mutex::new(BufWriter::new(tokio::io::stdout())));
    let processes = Arc::new(Mutex::new(HashMap::<String, RunningProcess>::new()));
    let mut lines = BufReader::new(tokio::io::stdin()).lines();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        let message = serde_json::from_str::<JSONRPCMessage>(&line)?;
        if let JSONRPCMessage::Request(request) = message {
            handle_request(request, &writer, &processes).await;
            continue;
        }

        if let JSONRPCMessage::Notification(notification) = message {
            if notification.method != INITIALIZED_METHOD {
                send_error(
                    &writer,
                    RequestId::Integer(-1),
                    invalid_request(format!(
                        "unexpected notification method: {}",
                        notification.method
                    )),
                )
                .await;
            }
            continue;
        }
    }

    let remaining = {
        let mut processes = processes.lock().await;
        processes
            .drain()
            .map(|(_, process)| process)
            .collect::<Vec<_>>()
    };
    for process in remaining {
        process.session.terminate();
    }

    Ok(())
}

async fn handle_request(
    request: JSONRPCRequest,
    writer: &Arc<Mutex<BufWriter<tokio::io::Stdout>>>,
    processes: &Arc<Mutex<HashMap<String, RunningProcess>>>,
) {
    let response = match request.method.as_str() {
        INITIALIZE_METHOD => serde_json::to_value(InitializeResponse {
            protocol_version: PROTOCOL_VERSION.to_string(),
        })
        .map_err(|err| internal_error(err.to_string())),
        EXEC_METHOD => handle_exec_request(request.params, writer, processes).await,
        EXEC_WRITE_METHOD => handle_write_request(request.params, processes).await,
        EXEC_TERMINATE_METHOD => handle_terminate_request(request.params, processes).await,
        other => Err(invalid_request(format!("unknown method: {other}"))),
    };

    match response {
        Ok(result) => {
            send_response(
                writer,
                JSONRPCResponse {
                    id: request.id,
                    result,
                },
            )
            .await;
        }
        Err(err) => {
            send_error(writer, request.id, err).await;
        }
    }
}

async fn handle_exec_request(
    params: Option<serde_json::Value>,
    writer: &Arc<Mutex<BufWriter<tokio::io::Stdout>>>,
    processes: &Arc<Mutex<HashMap<String, RunningProcess>>>,
) -> Result<serde_json::Value, JSONRPCErrorError> {
    let params: ExecParams = serde_json::from_value(params.unwrap_or(serde_json::Value::Null))
        .map_err(|err| invalid_params(err.to_string()))?;

    let (program, args) = params
        .argv
        .split_first()
        .ok_or_else(|| invalid_params("argv must not be empty".to_string()))?;

    let spawned = if params.tty {
        codex_utils_pty::spawn_pty_process(
            program,
            args,
            params.cwd.as_path(),
            &params.env,
            &params.arg0,
            TerminalSize::default(),
        )
        .await
    } else {
        codex_utils_pty::spawn_pipe_process_no_stdin(
            program,
            args,
            params.cwd.as_path(),
            &params.env,
            &params.arg0,
        )
        .await
    }
    .map_err(|err| internal_error(err.to_string()))?;

    let pid = spawned.session.pid();
    let stdout_buffer = Arc::new(StdMutex::new(BoundedBytesBuffer::new(
        params.output_bytes_cap,
    )));
    let stderr_buffer = Arc::new(StdMutex::new(BoundedBytesBuffer::new(
        params.output_bytes_cap,
    )));

    let process_id = params.process_id.clone();
    {
        let mut process_map = processes.lock().await;
        if process_map.contains_key(&process_id) {
            spawned.session.terminate();
            return Err(invalid_request(format!(
                "process {} already exists",
                params.process_id
            )));
        }
        process_map.insert(
            process_id.clone(),
            RunningProcess {
                session: spawned.session,
                tty: params.tty,
                stdout_buffer: Arc::clone(&stdout_buffer),
                stderr_buffer: Arc::clone(&stderr_buffer),
            },
        );
    }

    tokio::spawn(stream_output(
        process_id.clone(),
        ExecOutputStream::Stdout,
        spawned.stdout_rx,
        Arc::clone(writer),
        Arc::clone(&stdout_buffer),
    ));
    tokio::spawn(stream_output(
        process_id.clone(),
        ExecOutputStream::Stderr,
        spawned.stderr_rx,
        Arc::clone(writer),
        Arc::clone(&stderr_buffer),
    ));
    tokio::spawn(watch_exit(
        process_id.clone(),
        spawned.exit_rx,
        Arc::clone(writer),
        Arc::clone(processes),
    ));

    serde_json::to_value(ExecResponse {
        process_id,
        pid,
        running: true,
        exit_code: None,
        stdout: None,
        stderr: None,
    })
    .map_err(|err| internal_error(err.to_string()))
}

async fn handle_write_request(
    params: Option<serde_json::Value>,
    processes: &Arc<Mutex<HashMap<String, RunningProcess>>>,
) -> Result<serde_json::Value, JSONRPCErrorError> {
    let params: WriteParams = serde_json::from_value(params.unwrap_or(serde_json::Value::Null))
        .map_err(|err| invalid_params(err.to_string()))?;

    let writer_tx = {
        let process_map = processes.lock().await;
        let process = process_map
            .get(&params.process_id)
            .ok_or_else(|| invalid_request(format!("unknown process id {}", params.process_id)))?;
        if !process.tty {
            return Err(invalid_request(format!(
                "stdin is closed for process {}",
                params.process_id
            )));
        }
        process.session.writer_sender()
    };

    writer_tx
        .send(params.chunk.into_inner())
        .await
        .map_err(|_| internal_error("failed to write to process stdin".to_string()))?;

    serde_json::to_value(WriteResponse { accepted: true })
        .map_err(|err| internal_error(err.to_string()))
}

async fn handle_terminate_request(
    params: Option<serde_json::Value>,
    processes: &Arc<Mutex<HashMap<String, RunningProcess>>>,
) -> Result<serde_json::Value, JSONRPCErrorError> {
    let params: TerminateParams = serde_json::from_value(params.unwrap_or(serde_json::Value::Null))
        .map_err(|err| invalid_params(err.to_string()))?;

    let process = {
        let mut process_map = processes.lock().await;
        process_map.remove(&params.process_id)
    };

    if let Some(process) = process {
        process.session.terminate();
        serde_json::to_value(TerminateResponse { running: true })
            .map_err(|err| internal_error(err.to_string()))
    } else {
        serde_json::to_value(TerminateResponse { running: false })
            .map_err(|err| internal_error(err.to_string()))
    }
}

async fn stream_output(
    process_id: String,
    stream: ExecOutputStream,
    mut receiver: tokio::sync::mpsc::Receiver<Vec<u8>>,
    writer: Arc<Mutex<BufWriter<tokio::io::Stdout>>>,
    buffer: Arc<StdMutex<BoundedBytesBuffer>>,
) {
    while let Some(chunk) = receiver.recv().await {
        if let Ok(mut guard) = buffer.lock() {
            guard.push_chunk(&chunk);
        }
        let notification = ExecOutputDeltaNotification {
            process_id: process_id.clone(),
            stream,
            chunk: chunk.into(),
        };
        if send_notification(&writer, EXEC_OUTPUT_DELTA_METHOD, &notification)
            .await
            .is_err()
        {
            break;
        }
    }
}

async fn watch_exit(
    process_id: String,
    exit_rx: tokio::sync::oneshot::Receiver<i32>,
    writer: Arc<Mutex<BufWriter<tokio::io::Stdout>>>,
    processes: Arc<Mutex<HashMap<String, RunningProcess>>>,
) {
    let exit_code = exit_rx.await.unwrap_or(-1);
    let removed = {
        let mut processes = processes.lock().await;
        processes.remove(&process_id)
    };
    if let Some(process) = removed {
        let _ = process.stdout_buffer.lock().map(|buffer| buffer.snapshot());
        let _ = process.stderr_buffer.lock().map(|buffer| buffer.snapshot());
    }
    let _ = send_notification(
        &writer,
        EXEC_EXITED_METHOD,
        &ExecExitedNotification {
            process_id,
            exit_code,
        },
    )
    .await;
}

async fn send_response(
    writer: &Arc<Mutex<BufWriter<tokio::io::Stdout>>>,
    response: JSONRPCResponse,
) {
    let _ = send_message(writer, JSONRPCMessage::Response(response)).await;
}

async fn send_error(
    writer: &Arc<Mutex<BufWriter<tokio::io::Stdout>>>,
    id: RequestId,
    error: JSONRPCErrorError,
) {
    let _ = send_message(writer, JSONRPCMessage::Error(JSONRPCError { error, id })).await;
}

async fn send_notification<T: Serialize>(
    writer: &Arc<Mutex<BufWriter<tokio::io::Stdout>>>,
    method: &str,
    params: &T,
) -> Result<(), serde_json::Error> {
    send_message(
        writer,
        JSONRPCMessage::Notification(JSONRPCNotification {
            method: method.to_string(),
            params: Some(serde_json::to_value(params)?),
        }),
    )
    .await
    .map_err(serde_json::Error::io)
}

async fn send_message(
    writer: &Arc<Mutex<BufWriter<tokio::io::Stdout>>>,
    message: JSONRPCMessage,
) -> std::io::Result<()> {
    let encoded =
        serde_json::to_vec(&message).map_err(|err| std::io::Error::other(err.to_string()))?;
    let mut writer = writer.lock().await;
    writer.write_all(&encoded).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await
}

fn invalid_request(message: String) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: -32600,
        data: None,
        message,
    }
}

fn invalid_params(message: String) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: -32602,
        data: None,
        message,
    }
}

fn internal_error(message: String) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: -32603,
        data: None,
        message,
    }
}
