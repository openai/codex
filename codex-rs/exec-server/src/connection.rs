#[cfg(windows)]
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::extract::ws::Message as AxumWebSocketMessage;
use axum::extract::ws::WebSocket as AxumWebSocket;
use codex_app_server_protocol::JSONRPCMessage;
use futures::Sink;
use futures::SinkExt;
use futures::Stream;
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
#[cfg(test)]
pub(crate) const WEBSOCKET_KEEPALIVE_INTERVAL: Duration = Duration::from_millis(25);
#[cfg(not(test))]
pub(crate) const WEBSOCKET_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub(crate) enum JsonRpcConnectionEvent {
    Message(JSONRPCMessage),
    MalformedMessage { reason: String },
    Disconnected { reason: Option<String> },
}

#[derive(Clone)]
pub(crate) enum JsonRpcTransport {
    /// `Plain` means this connection owns no child process. It does not mean
    /// that the transport bytes are plaintext.
    Plain,
    Stdio {
        transport: StdioTransport,
    },
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
        Self::from_websocket_stream(stream, connection_label, /*ping_interval*/ None)
    }

    pub(crate) fn from_axum_websocket(stream: AxumWebSocket, connection_label: String) -> Self {
        Self::from_websocket_stream(stream, connection_label, Some(WEBSOCKET_KEEPALIVE_INTERVAL))
    }

    fn from_websocket_stream<T, M, E>(
        mut websocket: T,
        connection_label: String,
        ping_interval: Option<Duration>,
    ) -> Self
    where
        T: Sink<M, Error = E> + Stream<Item = Result<M, E>> + Unpin + Send + 'static,
        M: JsonRpcWebSocketMessage,
        E: std::fmt::Display + Send + 'static,
    {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (incoming_tx, incoming_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (disconnected_tx, disconnected_rx) = watch::channel(false);

        let websocket_task = tokio::spawn(async move {
            let mut ping_interval = ping_interval.map(|ping_interval| {
                let mut interval = tokio::time::interval_at(
                    tokio::time::Instant::now() + ping_interval,
                    ping_interval,
                );
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                interval
            });

            loop {
                tokio::select! {
                    maybe_message = outgoing_rx.recv() => {
                        let Some(message) = maybe_message else {
                            break;
                        };
                        if let Err(reason) = send_websocket_jsonrpc_message(
                            &mut websocket,
                            &connection_label,
                            &message,
                        )
                        .await
                        {
                            send_disconnected(&incoming_tx, &disconnected_tx, Some(reason)).await;
                            break;
                        }
                    }
                    _ = async {
                        match ping_interval.as_mut() {
                            Some(interval) => interval.tick().await,
                            None => std::future::pending().await,
                        }
                    } => {
                        if let Err(err) = websocket.send(M::ping()).await {
                            send_disconnected(
                                &incoming_tx,
                                &disconnected_tx,
                                Some(format!(
                                    "failed to write websocket ping to {connection_label}: {err}"
                                )),
                            )
                            .await;
                            break;
                        }
                    }
                    incoming_message = websocket.next() => {
                        match incoming_message {
                            Some(Ok(message)) => match message.parse_jsonrpc_frame() {
                                Ok(JsonRpcWebSocketFrame::Message(message)) => {
                                    if incoming_tx
                                        .send(JsonRpcConnectionEvent::Message(message))
                                        .await
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                                Ok(JsonRpcWebSocketFrame::Close) => {
                                    send_disconnected(
                                        &incoming_tx,
                                        &disconnected_tx,
                                        /*reason*/ None,
                                    )
                                    .await;
                                    break;
                                }
                                Ok(JsonRpcWebSocketFrame::Ignore) => {}
                                Err(err) => {
                                    send_malformed_message(
                                        &incoming_tx,
                                        Some(format!(
                                            "failed to parse websocket JSON-RPC message from {connection_label}: {err}"
                                        )),
                                    )
                                    .await;
                                }
                            },
                            Some(Err(err)) => {
                                send_disconnected(
                                    &incoming_tx,
                                    &disconnected_tx,
                                    Some(format!(
                                        "failed to read websocket JSON-RPC message from {connection_label}: {err}"
                                    )),
                                )
                                .await;
                                break;
                            }
                            None => {
                                send_disconnected(
                                    &incoming_tx,
                                    &disconnected_tx,
                                    /*reason*/ None,
                                )
                                .await;
                                break;
                            }
                        }
                    }
                }
            }
        });

        Self {
            outgoing_tx,
            incoming_rx,
            disconnected_rx,
            task_handles: vec![websocket_task],
            transport: JsonRpcTransport::Plain,
        }
    }

    pub(crate) fn with_child_process(mut self, child_process: Child) -> Self {
        self.transport = JsonRpcTransport::from_child_process(child_process);
        self
    }
}

enum JsonRpcWebSocketFrame {
    Message(JSONRPCMessage),
    Close,
    Ignore,
}

trait JsonRpcWebSocketMessage: Send + 'static {
    fn parse_jsonrpc_frame(self) -> Result<JsonRpcWebSocketFrame, serde_json::Error>;
    fn from_text(text: String) -> Self;
    fn ping() -> Self;
}

impl JsonRpcWebSocketMessage for Message {
    fn parse_jsonrpc_frame(self) -> Result<JsonRpcWebSocketFrame, serde_json::Error> {
        match self {
            Message::Text(text) => {
                serde_json::from_str(text.as_ref()).map(JsonRpcWebSocketFrame::Message)
            }
            Message::Binary(bytes) => {
                serde_json::from_slice(bytes.as_ref()).map(JsonRpcWebSocketFrame::Message)
            }
            Message::Close(_) => Ok(JsonRpcWebSocketFrame::Close),
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {
                Ok(JsonRpcWebSocketFrame::Ignore)
            }
        }
    }

    fn from_text(text: String) -> Self {
        Self::Text(text.into())
    }

    fn ping() -> Self {
        Self::Ping(Vec::new().into())
    }
}

impl JsonRpcWebSocketMessage for AxumWebSocketMessage {
    fn parse_jsonrpc_frame(self) -> Result<JsonRpcWebSocketFrame, serde_json::Error> {
        match self {
            AxumWebSocketMessage::Text(text) => {
                serde_json::from_str(text.as_ref()).map(JsonRpcWebSocketFrame::Message)
            }
            AxumWebSocketMessage::Binary(bytes) => {
                serde_json::from_slice(bytes.as_ref()).map(JsonRpcWebSocketFrame::Message)
            }
            AxumWebSocketMessage::Close(_) => Ok(JsonRpcWebSocketFrame::Close),
            AxumWebSocketMessage::Ping(_) | AxumWebSocketMessage::Pong(_) => {
                Ok(JsonRpcWebSocketFrame::Ignore)
            }
        }
    }

    fn from_text(text: String) -> Self {
        Self::Text(text.into())
    }

    fn ping() -> Self {
        Self::Ping(Vec::new().into())
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

async fn send_websocket_jsonrpc_message<W, M, E>(
    websocket_writer: &mut W,
    connection_label: &str,
    message: &JSONRPCMessage,
) -> Result<(), String>
where
    W: Sink<M, Error = E> + Unpin,
    M: JsonRpcWebSocketMessage,
    E: std::fmt::Display,
{
    match serialize_jsonrpc_message(message) {
        Ok(encoded) => websocket_writer
            .send(M::from_text(encoded))
            .await
            .map_err(|err| {
                format!("failed to write websocket JSON-RPC message to {connection_label}: {err}")
            }),
        Err(err) => Err(format!(
            "failed to serialize JSON-RPC message for {connection_label}: {err}"
        )),
    }
}

fn serialize_jsonrpc_message(message: &JSONRPCMessage) -> Result<String, serde_json::Error> {
    serde_json::to_string(message)
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use std::task::Context;
    use std::task::Poll;

    use codex_app_server_protocol::JSONRPCRequest;
    use codex_app_server_protocol::RequestId;
    use futures::channel::mpsc as futures_mpsc;
    use futures::task::AtomicWaker;
    use tokio::net::TcpListener;
    use tokio::time::timeout;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::connect_async;

    use super::*;

    #[tokio::test]
    async fn websocket_connection_sends_configured_ping() -> anyhow::Result<()> {
        let (client_websocket, mut server_websocket) = websocket_pair().await?;
        let connection = JsonRpcConnection::from_websocket_stream(
            client_websocket,
            "test".into(),
            Some(WEBSOCKET_KEEPALIVE_INTERVAL),
        );

        let message = timeout(Duration::from_secs(1), server_websocket.next())
            .await?
            .expect("websocket should stay open")?;
        assert!(matches!(message, Message::Ping(_)));

        drop(connection);
        Ok(())
    }

    #[tokio::test]
    async fn websocket_connection_ignores_server_pong() -> anyhow::Result<()> {
        let (client_websocket, mut server_websocket) = websocket_pair().await?;
        let mut connection = JsonRpcConnection::from_websocket(client_websocket, "test".into());

        server_websocket
            .send(Message::Pong(b"check".to_vec().into()))
            .await?;
        assert!(
            timeout(Duration::from_millis(50), connection.incoming_rx.recv())
                .await
                .is_err()
        );

        drop(connection);
        Ok(())
    }

    #[tokio::test]
    async fn websocket_connection_reports_server_close() -> anyhow::Result<()> {
        let (client_websocket, mut server_websocket) = websocket_pair().await?;
        let mut connection = JsonRpcConnection::from_websocket(client_websocket, "test".into());

        server_websocket.close(None).await?;
        assert!(matches!(
            timeout(Duration::from_secs(1), connection.incoming_rx.recv()).await?,
            Some(JsonRpcConnectionEvent::Disconnected { reason: None })
        ));

        drop(connection);
        Ok(())
    }

    #[tokio::test]
    async fn websocket_connection_accepts_binary_jsonrpc_message() -> anyhow::Result<()> {
        let (client_websocket, mut server_websocket) = websocket_pair().await?;
        let mut connection = JsonRpcConnection::from_websocket(client_websocket, "test".into());
        let message = JSONRPCMessage::Request(JSONRPCRequest {
            id: RequestId::Integer(1),
            method: "test".to_string(),
            params: None,
            trace: None,
        });

        server_websocket
            .send(Message::Binary(serde_json::to_vec(&message)?.into()))
            .await?;
        assert!(matches!(
            timeout(Duration::from_secs(1), connection.incoming_rx.recv()).await?,
            Some(JsonRpcConnectionEvent::Message(actual)) if actual == message
        ));

        drop(connection);
        Ok(())
    }

    #[tokio::test]
    async fn websocket_connection_keeps_outbound_message_while_send_is_backpressured()
    -> anyhow::Result<()> {
        let (websocket, control, mut outbound_rx) =
            ControlledWebSocket::new(/*write_ready*/ false);
        let mut connection = JsonRpcConnection::from_websocket_stream(
            websocket,
            "test".into(),
            /*ping_interval*/ None,
        );
        let message = test_jsonrpc_message();

        connection.outgoing_tx.send(message.clone()).await?;
        control.wait_for_blocked_write().await?;
        control.send_inbound(Message::Pong(b"check".to_vec().into()))?;
        assert!(
            timeout(Duration::from_millis(50), connection.incoming_rx.recv())
                .await
                .is_err()
        );

        control.set_write_ready();
        assert!(matches!(
            timeout(Duration::from_secs(1), outbound_rx.next()).await?,
            Some(Message::Text(text)) if serde_json::from_str::<JSONRPCMessage>(&text)? == message
        ));
        drop(connection);
        Ok(())
    }

    async fn websocket_pair() -> anyhow::Result<(
        WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        WebSocketStream<tokio::net::TcpStream>,
    )> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let websocket_url = format!("ws://{}", listener.local_addr()?);
        let server_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            accept_async(stream).await.map_err(anyhow::Error::from)
        });
        let (client_websocket, _) = connect_async(websocket_url).await?;
        let server_websocket = server_task.await??;
        Ok((client_websocket, server_websocket))
    }

    fn test_jsonrpc_message() -> JSONRPCMessage {
        JSONRPCMessage::Request(JSONRPCRequest {
            id: RequestId::Integer(1),
            method: "test".to_string(),
            params: None,
            trace: None,
        })
    }

    struct ControlledWebSocket {
        inbound_rx: futures_mpsc::UnboundedReceiver<Result<Message, std::convert::Infallible>>,
        outbound_tx: futures_mpsc::UnboundedSender<Message>,
        write_ready: Arc<AtomicBool>,
        write_blocked: Arc<AtomicBool>,
        write_blocked_waker: Arc<AtomicWaker>,
        write_waker: Arc<AtomicWaker>,
    }

    struct ControlledWebSocketHandle {
        inbound_tx: futures_mpsc::UnboundedSender<Result<Message, std::convert::Infallible>>,
        write_ready: Arc<AtomicBool>,
        write_blocked: Arc<AtomicBool>,
        write_blocked_waker: Arc<AtomicWaker>,
        write_waker: Arc<AtomicWaker>,
    }

    impl ControlledWebSocket {
        fn new(
            write_ready: bool,
        ) -> (
            Self,
            ControlledWebSocketHandle,
            futures_mpsc::UnboundedReceiver<Message>,
        ) {
            let (inbound_tx, inbound_rx) = futures_mpsc::unbounded();
            let (outbound_tx, outbound_rx) = futures_mpsc::unbounded();
            let write_ready = Arc::new(AtomicBool::new(write_ready));
            let write_blocked = Arc::new(AtomicBool::new(false));
            let write_blocked_waker = Arc::new(AtomicWaker::new());
            let write_waker = Arc::new(AtomicWaker::new());
            (
                Self {
                    inbound_rx,
                    outbound_tx,
                    write_ready: Arc::clone(&write_ready),
                    write_blocked: Arc::clone(&write_blocked),
                    write_blocked_waker: Arc::clone(&write_blocked_waker),
                    write_waker: Arc::clone(&write_waker),
                },
                ControlledWebSocketHandle {
                    inbound_tx,
                    write_ready,
                    write_blocked,
                    write_blocked_waker,
                    write_waker,
                },
                outbound_rx,
            )
        }
    }

    impl ControlledWebSocketHandle {
        fn send_inbound(&self, message: Message) -> anyhow::Result<()> {
            self.inbound_tx
                .unbounded_send(Ok(message))
                .map_err(anyhow::Error::from)
        }

        fn set_write_ready(&self) {
            self.write_ready.store(true, Ordering::Release);
            self.write_waker.wake();
        }

        async fn wait_for_blocked_write(&self) -> anyhow::Result<()> {
            timeout(
                Duration::from_secs(1),
                futures::future::poll_fn(|cx| {
                    if self.write_blocked.load(Ordering::Acquire) {
                        Poll::Ready(())
                    } else {
                        self.write_blocked_waker.register(cx.waker());
                        Poll::Pending
                    }
                }),
            )
            .await?;
            Ok(())
        }
    }

    impl Sink<Message> for ControlledWebSocket {
        type Error = std::convert::Infallible;

        fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            if self.write_ready.load(Ordering::Acquire) {
                Poll::Ready(Ok(()))
            } else {
                self.write_blocked.store(true, Ordering::Release);
                self.write_blocked_waker.wake();
                self.write_waker.register(cx.waker());
                Poll::Pending
            }
        }

        fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
            self.outbound_tx
                .unbounded_send(item)
                .expect("test outbound receiver should stay open");
            Ok(())
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    impl Stream for ControlledWebSocket {
        type Item = Result<Message, std::convert::Infallible>;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            Pin::new(&mut self.inbound_rx).poll_next(cx)
        }
    }
}
