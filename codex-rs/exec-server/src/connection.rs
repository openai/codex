#[cfg(windows)]
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_app_server_protocol::JSONRPCMessage;
use futures::SinkExt;
use futures::StreamExt;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::process::Child;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tracing::debug;
use tracing::warn;

use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::BufWriter;

pub(crate) const CHANNEL_CAPACITY: usize = 128;
const STDIO_TERMINATION_GRACE_PERIOD: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub(crate) enum JsonRpcConnectionEvent {
    Message(JSONRPCMessage),
    MalformedMessage { reason: String },
    Disconnected { reason: Option<String> },
}

#[derive(Clone)]
pub(crate) enum JsonRpcTransport {
    Plain,
    Stdio { transport: StdioTransport },
}

impl JsonRpcTransport {
    fn from_child_process(child_process: Child) -> Self {
        Self::Stdio {
            transport: StdioTransport::spawn(child_process),
        }
    }

    pub(crate) fn terminate(&self) {
        match self {
            Self::Plain => {}
            Self::Stdio { transport } => transport.terminate(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct StdioTransport {
    handle: Arc<StdioTransportHandle>,
}

struct StdioTransportHandle {
    terminate_tx: watch::Sender<bool>,
    terminate_requested: AtomicBool,
}

impl StdioTransport {
    fn spawn(child_process: Child) -> Self {
        let (terminate_tx, terminate_rx) = watch::channel(false);
        let handle = Arc::new(StdioTransportHandle {
            terminate_tx,
            terminate_requested: AtomicBool::new(false),
        });
        spawn_stdio_child_supervisor(child_process, terminate_rx);
        Self { handle }
    }

    fn terminate(&self) {
        self.handle.terminate();
    }
}

impl StdioTransportHandle {
    fn terminate(&self) {
        if !self.terminate_requested.swap(true, Ordering::AcqRel) {
            let _ = self.terminate_tx.send(true);
        }
    }
}

impl Drop for StdioTransportHandle {
    fn drop(&mut self) {
        self.terminate();
    }
}

fn spawn_stdio_child_supervisor(mut child_process: Child, mut terminate_rx: watch::Receiver<bool>) {
    let process_group_id = child_process.id();
    tokio::spawn(async move {
        tokio::select! {
            result = child_process.wait() => {
                log_stdio_child_wait_result(result);
                kill_process_tree(&mut child_process, process_group_id);
            }
            () = wait_for_stdio_termination(&mut terminate_rx) => {
                terminate_stdio_child(&mut child_process, process_group_id).await;
            }
        }
    });
}

async fn wait_for_stdio_termination(terminate_rx: &mut watch::Receiver<bool>) {
    loop {
        if *terminate_rx.borrow() {
            return;
        }
        if terminate_rx.changed().await.is_err() {
            return;
        }
    }
}

async fn terminate_stdio_child(child_process: &mut Child, process_group_id: Option<u32>) {
    terminate_process_tree(child_process, process_group_id);
    match timeout(STDIO_TERMINATION_GRACE_PERIOD, child_process.wait()).await {
        Ok(result) => {
            log_stdio_child_wait_result(result);
        }
        Err(_) => {
            kill_process_tree(child_process, process_group_id);
            log_stdio_child_wait_result(child_process.wait().await);
        }
    }
}

fn terminate_process_tree(child_process: &mut Child, process_group_id: Option<u32>) {
    let Some(process_group_id) = process_group_id else {
        kill_direct_child(child_process, "terminate");
        return;
    };

    #[cfg(unix)]
    if let Err(err) = codex_utils_pty::process_group::terminate_process_group(process_group_id) {
        warn!("failed to terminate exec-server stdio process group {process_group_id}: {err}");
        kill_direct_child(child_process, "terminate");
    }

    #[cfg(windows)]
    if !kill_windows_process_tree(process_group_id) {
        kill_direct_child(child_process, "terminate");
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = process_group_id;
        kill_direct_child(child_process, "terminate");
    }
}

fn kill_process_tree(child_process: &mut Child, process_group_id: Option<u32>) {
    let Some(process_group_id) = process_group_id else {
        kill_direct_child(child_process, "kill");
        return;
    };

    #[cfg(unix)]
    if let Err(err) = codex_utils_pty::process_group::kill_process_group(process_group_id) {
        warn!("failed to kill exec-server stdio process group {process_group_id}: {err}");
    }

    #[cfg(windows)]
    if !kill_windows_process_tree(process_group_id) {
        kill_direct_child(child_process, "kill");
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = process_group_id;
        kill_direct_child(child_process, "kill");
    }
}

fn kill_direct_child(child_process: &mut Child, action: &str) {
    if let Err(err) = child_process.start_kill() {
        debug!("failed to {action} exec-server stdio child: {err}");
    }
}

#[cfg(windows)]
fn kill_windows_process_tree(pid: u32) -> bool {
    let pid = pid.to_string();
    match std::process::Command::new("taskkill")
        .args(["/PID", pid.as_str(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) => status.success(),
        Err(err) => {
            warn!("failed to run taskkill for exec-server stdio process tree {pid}: {err}");
            false
        }
    }
}

fn log_stdio_child_wait_result(result: std::io::Result<std::process::ExitStatus>) {
    if let Err(err) = result {
        debug!("failed to wait for exec-server stdio child: {err}");
    }
}

pub(crate) struct JsonRpcConnection {
    pub(crate) outgoing_tx: mpsc::Sender<JSONRPCMessage>,
    pub(crate) incoming_rx: mpsc::Receiver<JsonRpcConnectionEvent>,
    pub(crate) disconnected_rx: watch::Receiver<bool>,
    pub(crate) task_handles: Vec<tokio::task::JoinHandle<()>>,
    pub(crate) transport: JsonRpcTransport,
}

impl JsonRpcConnection {
    pub(crate) fn from_stdio<R, W>(reader: R, writer: W, connection_label: String) -> Self
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (incoming_tx, incoming_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (disconnected_tx, disconnected_rx) = watch::channel(false);

        let reader_label = connection_label.clone();
        let incoming_tx_for_reader = incoming_tx.clone();
        let disconnected_tx_for_reader = disconnected_tx.clone();
        let reader_task = tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        if line.trim().is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<JSONRPCMessage>(&line) {
                            Ok(message) => {
                                if incoming_tx_for_reader
                                    .send(JsonRpcConnectionEvent::Message(message))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(err) => {
                                send_malformed_message(
                                    &incoming_tx_for_reader,
                                    Some(format!(
                                        "failed to parse JSON-RPC message from {reader_label}: {err}"
                                    )),
                                )
                                .await;
                            }
                        }
                    }
                    Ok(None) => {
                        send_disconnected(
                            &incoming_tx_for_reader,
                            &disconnected_tx_for_reader,
                            /*reason*/ None,
                        )
                        .await;
                        break;
                    }
                    Err(err) => {
                        send_disconnected(
                            &incoming_tx_for_reader,
                            &disconnected_tx_for_reader,
                            Some(format!(
                                "failed to read JSON-RPC message from {reader_label}: {err}"
                            )),
                        )
                        .await;
                        break;
                    }
                }
            }
        });

        let writer_task = tokio::spawn(async move {
            let mut writer = BufWriter::new(writer);
            while let Some(message) = outgoing_rx.recv().await {
                if let Err(err) = write_jsonrpc_line_message(&mut writer, &message).await {
                    send_disconnected(
                        &incoming_tx,
                        &disconnected_tx,
                        Some(format!(
                            "failed to write JSON-RPC message to {connection_label}: {err}"
                        )),
                    )
                    .await;
                    break;
                }
            }
        });

        Self {
            outgoing_tx,
            incoming_rx,
            disconnected_rx,
            task_handles: vec![reader_task, writer_task],
            transport: JsonRpcTransport::Plain,
        }
    }

    pub(crate) fn from_websocket<S>(stream: WebSocketStream<S>, connection_label: String) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (incoming_tx, incoming_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (disconnected_tx, disconnected_rx) = watch::channel(false);
        let (mut websocket_writer, mut websocket_reader) = stream.split();

        let reader_label = connection_label.clone();
        let incoming_tx_for_reader = incoming_tx.clone();
        let disconnected_tx_for_reader = disconnected_tx.clone();
        let reader_task = tokio::spawn(async move {
            loop {
                match websocket_reader.next().await {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<JSONRPCMessage>(text.as_ref()) {
                            Ok(message) => {
                                if incoming_tx_for_reader
                                    .send(JsonRpcConnectionEvent::Message(message))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(err) => {
                                send_malformed_message(
                                    &incoming_tx_for_reader,
                                    Some(format!(
                                        "failed to parse websocket JSON-RPC message from {reader_label}: {err}"
                                    )),
                                )
                                .await;
                            }
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        match serde_json::from_slice::<JSONRPCMessage>(bytes.as_ref()) {
                            Ok(message) => {
                                if incoming_tx_for_reader
                                    .send(JsonRpcConnectionEvent::Message(message))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(err) => {
                                send_malformed_message(
                                    &incoming_tx_for_reader,
                                    Some(format!(
                                        "failed to parse websocket JSON-RPC message from {reader_label}: {err}"
                                    )),
                                )
                                .await;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        send_disconnected(
                            &incoming_tx_for_reader,
                            &disconnected_tx_for_reader,
                            /*reason*/ None,
                        )
                        .await;
                        break;
                    }
                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(_)) => {}
                    Some(Err(err)) => {
                        send_disconnected(
                            &incoming_tx_for_reader,
                            &disconnected_tx_for_reader,
                            Some(format!(
                                "failed to read websocket JSON-RPC message from {reader_label}: {err}"
                            )),
                        )
                        .await;
                        break;
                    }
                    None => {
                        send_disconnected(
                            &incoming_tx_for_reader,
                            &disconnected_tx_for_reader,
                            /*reason*/ None,
                        )
                        .await;
                        break;
                    }
                }
            }
        });

        let writer_task = tokio::spawn(async move {
            while let Some(message) = outgoing_rx.recv().await {
                match serialize_jsonrpc_message(&message) {
                    Ok(encoded) => {
                        if let Err(err) = websocket_writer.send(Message::Text(encoded.into())).await
                        {
                            send_disconnected(
                                &incoming_tx,
                                &disconnected_tx,
                                Some(format!(
                                    "failed to write websocket JSON-RPC message to {connection_label}: {err}"
                                )),
                            )
                            .await;
                            break;
                        }
                    }
                    Err(err) => {
                        send_disconnected(
                            &incoming_tx,
                            &disconnected_tx,
                            Some(format!(
                                "failed to serialize JSON-RPC message for {connection_label}: {err}"
                            )),
                        )
                        .await;
                        break;
                    }
                }
            }
        });

        Self {
            outgoing_tx,
            incoming_rx,
            disconnected_rx,
            task_handles: vec![reader_task, writer_task],
            transport: JsonRpcTransport::Plain,
        }
    }

    pub(crate) fn with_child_process(mut self, child_process: Child) -> Self {
        self.transport = JsonRpcTransport::from_child_process(child_process);
        self
    }
}

async fn send_disconnected(
    incoming_tx: &mpsc::Sender<JsonRpcConnectionEvent>,
    disconnected_tx: &watch::Sender<bool>,
    reason: Option<String>,
) {
    let _ = disconnected_tx.send(true);
    let _ = incoming_tx
        .send(JsonRpcConnectionEvent::Disconnected { reason })
        .await;
}

async fn send_malformed_message(
    incoming_tx: &mpsc::Sender<JsonRpcConnectionEvent>,
    reason: Option<String>,
) {
    let _ = incoming_tx
        .send(JsonRpcConnectionEvent::MalformedMessage {
            reason: reason.unwrap_or_else(|| "malformed JSON-RPC message".to_string()),
        })
        .await;
}

async fn write_jsonrpc_line_message<W>(
    writer: &mut BufWriter<W>,
    message: &JSONRPCMessage,
) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let encoded =
        serialize_jsonrpc_message(message).map_err(|err| std::io::Error::other(err.to_string()))?;
    writer.write_all(encoded.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await
}

fn serialize_jsonrpc_message(message: &JSONRPCMessage) -> Result<String, serde_json::Error> {
    serde_json::to_string(message)
}
