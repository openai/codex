use std::fmt;
use std::future::Future;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeSessionCellExecutionLimits;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::StartedCell;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::WaitRequest;
use codex_code_mode_protocol::host::Capability;
use codex_code_mode_protocol::host::CapabilitySet;
use codex_code_mode_protocol::host::ClientHello;
use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::DUAL_WEBSOCKET_CAPABILITY;
use codex_code_mode_protocol::host::EncodedFrame;
use codex_code_mode_protocol::host::FramedReader;
use codex_code_mode_protocol::host::FramedWriter;
use codex_code_mode_protocol::host::HostToClient;
use codex_code_mode_protocol::host::MAX_FRAME_BYTES;
use codex_code_mode_protocol::host::MAX_PENDING_DELEGATE_CALLS;
use codex_code_mode_protocol::host::ProtocolVersion;
use codex_code_mode_protocol::host::RequestId;
use codex_code_mode_protocol::host::SESSION_RESOURCE_LIMITS_CAPABILITY;
use codex_code_mode_protocol::host::SupportedProtocolVersions;
use codex_code_mode_protocol::host::TransportLane;
use codex_http_client::HttpClientFactory;
use codex_protocol::shell_environment::scrub_non_inheritable_env_vars;
use codex_websocket_client::WebSocketConnector;
use futures::StreamExt;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::Uri;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::warn;

use self::driver::ConnectionDriver;
use self::driver::DriverCommand;
use self::driver::DriverEvent;
use self::driver::DriverLifecycle;
pub(super) use self::driver::RemoteSession;
pub(super) use self::driver::SessionCleanup;
use self::reader::drive_reader;
use self::transport::ConnectionReader;
use self::transport::ConnectionWriter;

mod driver;
mod reader;
mod transport;

const IPC_CHANNEL_CAPACITY: usize = 128;
const HOST_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const LOCAL_HOST_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
// TODO(anp) make this timeout configurable if 60 seconds is insufficient.
const DEFAULT_HOST_WAIT_TRANSPORT_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_WEBSOCKET_FRAME_BYTES: usize = MAX_FRAME_BYTES + std::mem::size_of::<u32>();
// Host spawn errors become model-visible tool output. Bound configured paths
// while preserving the executable-bearing suffix needed to diagnose failures.
const MAX_DISPLAYED_HOST_PROGRAM_BYTES: usize = 512;
const TRUNCATED_HOST_PROGRAM_PREFIX: &str = "...";

pub(super) enum ConnectionError {
    Spawn {
        host_program: PathBuf,
        error: io::Error,
    },
    BulkConnectionUnavailable(String),
    Other(String),
}

impl fmt::Display for ConnectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn {
                host_program,
                error,
            } => {
                let host_program = host_program.to_string_lossy();
                if host_program.len() <= MAX_DISPLAYED_HOST_PROGRAM_BYTES {
                    return write!(
                        formatter,
                        "failed to spawn code-mode host {host_program}: {error}"
                    );
                }

                let mut suffix_start = host_program.len()
                    - (MAX_DISPLAYED_HOST_PROGRAM_BYTES - TRUNCATED_HOST_PROGRAM_PREFIX.len());
                while !host_program.is_char_boundary(suffix_start) {
                    suffix_start += 1;
                }

                write!(
                    formatter,
                    "failed to spawn code-mode host {TRUNCATED_HOST_PROGRAM_PREFIX}{}: {error}",
                    &host_program[suffix_start..]
                )
            }
            Self::BulkConnectionUnavailable(message) | Self::Other(message) => {
                formatter.write_str(message)
            }
        }
    }
}

pub(super) struct Connection {
    command_tx: mpsc::Sender<DriverCommand>,
    execute_claim_tx: mpsc::UnboundedSender<RequestId>,
    alive: Arc<AtomicBool>,
    failure: Arc<std::sync::Mutex<Option<String>>>,
    cancellation: CancellationToken,
    capabilities: CapabilitySet,
}

struct CallerCancellation {
    token: CancellationToken,
    armed: bool,
}

struct ConnectionSupervisor {
    owner: ConnectionOwner,
    event_tx: mpsc::Sender<DriverEvent>,
    cancellation: CancellationToken,
    alive: Arc<AtomicBool>,
    failure: Arc<std::sync::Mutex<Option<String>>>,
    driver_task: JoinHandle<()>,
    reader_task: JoinHandle<Result<(), String>>,
    writer_task: JoinHandle<Result<(), String>>,
}

enum ConnectionOwner {
    Process(Box<Child>),
    WebSocket,
}

struct BulkConnectionOptions {
    websocket_url: String,
    http_client_factory: HttpClientFactory,
}

impl BulkConnectionOptions {
    async fn connect(
        &self,
        token: &str,
    ) -> Result<(ConnectionReader, ConnectionWriter), ConnectionError> {
        let control_uri = self.websocket_url.parse::<Uri>().map_err(|error| {
            ConnectionError::Other(format!(
                "failed to build code-mode host bulk websocket URL: {error}"
            ))
        })?;
        let bulk_path = format!("{}/bulk/{token}", control_uri.path().trim_end_matches('/'));
        let bulk_path_and_query = match control_uri.query() {
            Some(query) => format!("{bulk_path}?{query}"),
            None => bulk_path,
        };
        let mut bulk_uri_parts = control_uri.into_parts();
        bulk_uri_parts.path_and_query = Some(bulk_path_and_query.parse().map_err(|error| {
            ConnectionError::Other(format!(
                "failed to build code-mode host bulk websocket path: {error}"
            ))
        })?);
        let bulk_url = Uri::from_parts(bulk_uri_parts)
            .map_err(|error| {
                ConnectionError::Other(format!(
                    "failed to build code-mode host bulk websocket URL: {error}"
                ))
            })?
            .to_string();
        connect_websocket_transport(&bulk_url, &self.http_client_factory)
            .await
            .map_err(|error| ConnectionError::BulkConnectionUnavailable(error.to_string()))
    }
}

impl CallerCancellation {
    fn new() -> Self {
        Self {
            token: CancellationToken::new(),
            armed: true,
        }
    }

    fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for CallerCancellation {
    fn drop(&mut self) {
        if self.armed {
            self.token.cancel();
        }
    }
}

impl Connection {
    pub(super) async fn spawn(host_program: &Path) -> Result<Self, ConnectionError> {
        let mut command = Command::new(host_program);
        #[cfg(unix)]
        command.process_group(0);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        scrub_non_inheritable_env_vars(command.as_std_mut());
        let mut child = command.spawn().map_err(|error| ConnectionError::Spawn {
            host_program: host_program.to_path_buf(),
            error,
        })?;

        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                loop {
                    match lines.next_line().await {
                        Ok(Some(line)) => debug!("code-mode host stderr: {line}"),
                        Ok(None) => break,
                        Err(err) => {
                            warn!("failed to read code-mode host stderr: {err}");
                            break;
                        }
                    }
                }
            });
        }

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| ConnectionError::Other("spawned code-mode host has no stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ConnectionError::Other("spawned code-mode host has no stdout".into()))?;

        Self::establish(
            ConnectionReader::Stdio(FramedReader::new(stdout)),
            ConnectionWriter::Stdio(FramedWriter::new(stdin)),
            ConnectionOwner::Process(Box::new(child)),
            /*bulk_connection_options*/ None,
        )
        .await
    }

    pub(super) async fn connect_websocket(
        websocket_url: &str,
        http_client_factory: &HttpClientFactory,
    ) -> Result<Self, ConnectionError> {
        let (reader, writer) =
            connect_websocket_transport(websocket_url, http_client_factory).await?;

        Self::establish(
            reader,
            writer,
            ConnectionOwner::WebSocket,
            Some(BulkConnectionOptions {
                websocket_url: websocket_url.to_string(),
                http_client_factory: http_client_factory.clone(),
            }),
        )
        .await
    }

    async fn establish(
        mut reader: ConnectionReader,
        mut writer: ConnectionWriter,
        mut owner: ConnectionOwner,
        bulk_connection_options: Option<BulkConnectionOptions>,
    ) -> Result<Self, ConnectionError> {
        let handshake = async {
            let dual_capability =
                Capability::new(DUAL_WEBSOCKET_CAPABILITY).map_err(|error| error.to_string())?;
            let session_limits_capability = Capability::new(SESSION_RESOURCE_LIMITS_CAPABILITY)
                .map_err(|error| error.to_string())?;
            let optional_capabilities = if bulk_connection_options.is_some() {
                CapabilitySet::try_new([dual_capability.clone(), session_limits_capability])
                    .map_err(|error| error.to_string())?
            } else {
                CapabilitySet::try_new([session_limits_capability])
                    .map_err(|error| error.to_string())?
            };
            let hello = ClientHello::new(
                SupportedProtocolVersions::try_new([ProtocolVersion::V1])
                    .map_err(|err| err.to_string())?,
                CapabilitySet::empty(),
                optional_capabilities,
            )
            .map_err(|err| err.to_string())?;
            writer
                .write(&ClientToHost::ClientHello(hello))
                .await
                .map_err(|err| format!("failed to write code-mode host hello: {err}"))?;
            match reader
                .read()
                .await
                .map_err(|err| format!("failed to read code-mode host hello: {err}"))?
            {
                Some(HostToClient::HostHello(hello))
                    if hello.selected_version() == ProtocolVersion::V1 =>
                {
                    let capabilities = hello.capabilities().clone();
                    let bulk_token = if capabilities.contains(&dual_capability) {
                        hello
                            .bulk_connection_token()
                            .map(str::to_string)
                            .ok_or_else(|| {
                                "code-mode host advertised dual websockets without a pairing token"
                                    .to_string()
                            })
                            .map(Some)?
                    } else if hello.bulk_connection_token().is_some() {
                        return Err(
                            "code-mode host returned an unexpected bulk pairing token".to_string()
                        );
                    } else {
                        None
                    };
                    Ok((capabilities, bulk_token))
                }
                Some(HostToClient::HandshakeRejected { reason }) => {
                    Err(format!("code-mode host rejected the handshake: {reason:?}"))
                }
                Some(message) => Err(format!(
                    "code-mode host returned an invalid handshake response: {message:?}"
                )),
                None => Err("code-mode host exited during handshake".to_string()),
            }
        };
        let handshake_timeout = match &owner {
            ConnectionOwner::Process(_) => LOCAL_HOST_STARTUP_TIMEOUT,
            ConnectionOwner::WebSocket => HOST_HANDSHAKE_TIMEOUT,
        };
        let handshake_result = match tokio::time::timeout(handshake_timeout, handshake).await {
            Ok(result) => result,
            Err(_) => {
                let _ = writer.close().await;
                owner.close().await;
                return Err(ConnectionError::Other(
                    "timed out negotiating with the code-mode host".into(),
                ));
            }
        };
        let (capabilities, bulk_token) = match handshake_result {
            Ok(negotiated) => negotiated,
            Err(err) => {
                let _ = writer.close().await;
                owner.close().await;
                return Err(ConnectionError::Other(err));
            }
        };
        let (bulk_reader, bulk_writer) = if let Some(token) = bulk_token {
            let Some(options) = bulk_connection_options else {
                let _ = writer.close().await;
                owner.close().await;
                return Err(ConnectionError::Other(
                    "code-mode host negotiated an unsupported bulk websocket".to_string(),
                ));
            };
            match options.connect(&token).await {
                Ok((reader, writer)) => (Some(reader), Some(writer)),
                Err(error) => {
                    let _ = writer.close().await;
                    owner.close().await;
                    return Err(error);
                }
            }
        } else {
            (None, None)
        };

        let (command_tx, command_rx) = mpsc::channel(IPC_CHANNEL_CAPACITY);
        let (event_tx, event_rx) = mpsc::channel(IPC_CHANNEL_CAPACITY);
        let (outgoing_tx, outgoing_rx) = mpsc::channel::<EncodedFrame>(IPC_CHANNEL_CAPACITY);
        let (bulk_tx, bulk_rx) = if bulk_writer.is_some() {
            let (sender, receiver) = mpsc::channel::<EncodedFrame>(MAX_PENDING_DELEGATE_CALLS);
            (Some(sender), Some(receiver))
        } else {
            (None, None)
        };
        let dual_websocket = bulk_writer.is_some();
        let cancellation = CancellationToken::new();
        let alive = Arc::new(AtomicBool::new(true));
        let failure = Arc::new(std::sync::Mutex::new(None));

        let writer_cancellation = cancellation.clone();
        let writer_task = tokio::spawn(async move {
            if let (Some(bulk_writer), Some(bulk_rx)) = (bulk_writer, bulk_rx) {
                tokio::try_join!(
                    drive_writer(writer, outgoing_rx, writer_cancellation.clone()),
                    drive_writer(bulk_writer, bulk_rx, writer_cancellation)
                )?;
                Ok(())
            } else {
                drive_writer(writer, outgoing_rx, writer_cancellation).await
            }
        });

        let reader_events = event_tx.clone();
        let reader_cancellation = cancellation.clone();
        let reader_task = tokio::spawn(async move {
            let lane = dual_websocket.then_some(TransportLane::Control);
            if let Some(bulk_reader) = bulk_reader {
                tokio::try_join!(
                    drive_reader(
                        reader,
                        reader_events.clone(),
                        reader_cancellation.clone(),
                        lane,
                    ),
                    drive_reader(
                        bulk_reader,
                        reader_events,
                        reader_cancellation,
                        Some(TransportLane::Bulk),
                    )
                )?;
                Ok(())
            } else {
                drive_reader(reader, reader_events, reader_cancellation, lane).await
            }
        });

        let (driver, execute_claim_tx) = ConnectionDriver::new(
            command_rx,
            event_rx,
            event_tx.clone(),
            outgoing_tx,
            DriverLifecycle {
                alive: Arc::clone(&alive),
                failure: Arc::clone(&failure),
                cancellation: cancellation.clone(),
            },
        );
        let driver = match bulk_tx {
            Some(sender) => driver.with_bulk_sender(sender),
            None => driver,
        };
        let driver_task = tokio::spawn(driver.run());
        tokio::spawn(
            ConnectionSupervisor {
                owner,
                event_tx,
                cancellation: cancellation.clone(),
                alive: Arc::clone(&alive),
                failure: Arc::clone(&failure),
                driver_task,
                reader_task,
                writer_task,
            }
            .run(),
        );

        Ok(Self {
            command_tx,
            execute_claim_tx,
            alive,
            failure,
            cancellation,
            capabilities,
        })
    }

    pub(super) fn is_alive(&self) -> bool {
        if self.command_tx.is_closed() {
            mark_connection_dead(
                &self.alive,
                &self.failure,
                "code-mode connection driver closed".to_string(),
            );
        }
        self.alive.load(Ordering::Acquire)
    }

    pub(super) async fn open_session(
        &self,
        session: RemoteSession,
        delegate: Arc<dyn CodeModeSessionDelegate>,
        limits: CodeModeSessionCellExecutionLimits,
    ) -> Result<SessionCleanup, String> {
        if limits != CodeModeSessionCellExecutionLimits::default()
            && !self
                .capabilities
                .iter()
                .any(|capability| capability.as_str() == SESSION_RESOURCE_LIMITS_CAPABILITY)
        {
            return Err(format!(
                "code-mode host does not support session resource limits: missing `{SESSION_RESOURCE_LIMITS_CAPABILITY}` capability"
            ));
        }
        let cleanup = SessionCleanup::new();
        let cancellation = CallerCancellation::new();
        let (response_tx, response_rx) = oneshot::channel();
        self.send(DriverCommand::OpenSession {
            session,
            delegate,
            limits,
            cleanup: cleanup.clone(),
            caller_cancellation: cancellation.token(),
            response_tx,
        })
        .await?;
        let result = self.receive(response_rx).await;
        cancellation.disarm();
        result?;
        Ok(cleanup)
    }

    pub(super) async fn execute(
        &self,
        session: RemoteSession,
        request: ExecuteRequest,
    ) -> Result<StartedCell, String> {
        let cancellation = CallerCancellation::new();
        let (response_tx, response_rx) = oneshot::channel();
        self.send(DriverCommand::Execute {
            session,
            request,
            caller_cancellation: cancellation.token(),
            response_tx,
        })
        .await?;
        let delivered = match self.receive(response_rx).await {
            Ok(delivered) => delivered,
            Err(err) => {
                cancellation.disarm();
                return Err(err);
            }
        };
        self.execute_claim_tx
            .send(delivered.request_id)
            .map_err(|_| self.failure_message())?;
        cancellation.disarm();
        Ok(delivered.started)
    }

    pub(super) async fn wait(
        &self,
        session: RemoteSession,
        request: WaitRequest,
    ) -> Result<WaitOutcome, String> {
        // Account for the runtime's one-second yield grace separately from transport.
        let runtime_timeout =
            Duration::from_millis(request.yield_time_ms).saturating_add(Duration::from_secs(1));
        let cancellation = CallerCancellation::new();
        let (response_tx, response_rx) = oneshot::channel();
        let result = self
            .with_transport_deadline(runtime_timeout, "wait", async {
                self.send(DriverCommand::Wait {
                    session,
                    request,
                    caller_cancellation: cancellation.token(),
                    response_tx,
                })
                .await?;
                self.receive(response_rx).await
            })
            .await;
        cancellation.disarm();
        result
    }

    pub(super) async fn terminate(
        &self,
        session: RemoteSession,
        cell_id: CellId,
    ) -> Result<WaitOutcome, String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.with_transport_deadline(Duration::ZERO, "terminate", async {
            self.send(DriverCommand::Terminate {
                session,
                cell_id,
                response_tx,
            })
            .await?;
            self.receive(response_rx).await
        })
        .await
    }

    pub(super) async fn shutdown_session(&self, session: RemoteSession) -> Result<(), String> {
        let (response_tx, response_rx) = oneshot::channel();
        self.send(DriverCommand::ShutdownSession {
            session,
            response_tx,
        })
        .await?;
        self.receive(response_rx).await
    }

    async fn with_transport_deadline<T>(
        &self,
        runtime_timeout: Duration,
        request_type: &str,
        request: impl Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        let deadline = runtime_timeout.saturating_add(DEFAULT_HOST_WAIT_TRANSPORT_TIMEOUT);
        match tokio::time::timeout(deadline, request).await {
            Ok(result) => result,
            Err(_) => {
                warn!(request_type, "code-mode host request exceeded its deadline");
                let reason =
                    format!("code-mode host timed out waiting for {request_type} response");
                mark_connection_dead(&self.alive, &self.failure, reason.clone());
                self.cancellation.cancel();
                Err(reason)
            }
        }
    }

    async fn send(&self, command: DriverCommand) -> Result<(), String> {
        if !self.is_alive() {
            return Err(self.failure_message());
        }
        self.command_tx
            .send(command)
            .await
            .map_err(|_| self.failure_message())
    }

    async fn receive<T>(
        &self,
        response_rx: oneshot::Receiver<Result<T, String>>,
    ) -> Result<T, String> {
        response_rx.await.map_err(|_| self.failure_message())?
    }

    fn failure_message(&self) -> String {
        self.failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .unwrap_or_else(|| "code-mode host connection closed".to_string())
    }
}

async fn connect_websocket_transport(
    websocket_url: &str,
    http_client_factory: &HttpClientFactory,
) -> Result<(ConnectionReader, ConnectionWriter), ConnectionError> {
    let request = websocket_url.into_client_request().map_err(|error| {
        ConnectionError::Other(format!(
            "failed to build code-mode host websocket request: {error}"
        ))
    })?;
    let connector = WebSocketConnector::new(http_client_factory)
        .map_err(|error| {
            ConnectionError::Other(format!(
                "failed to configure code-mode host websocket TLS: {error}"
            ))
        })?
        .with_tcp_nodelay();
    let websocket_config = WebSocketConfig::default()
        .max_frame_size(Some(MAX_WEBSOCKET_FRAME_BYTES))
        .max_message_size(Some(MAX_WEBSOCKET_FRAME_BYTES));
    let (websocket, _) = tokio::time::timeout(
        HOST_HANDSHAKE_TIMEOUT,
        connector.connect(request, websocket_config),
    )
    .await
    .map_err(|_| {
        ConnectionError::Other("timed out connecting to the code-mode host websocket".into())
    })?
    .map_err(|error| {
        ConnectionError::Other(format!(
            "failed to connect to the code-mode host websocket: {error}"
        ))
    })?;
    let (writer, reader) = websocket.split();
    Ok((
        ConnectionReader::WebSocket(reader),
        ConnectionWriter::WebSocket(writer),
    ))
}

async fn drive_writer(
    mut writer: ConnectionWriter,
    mut outgoing: mpsc::Receiver<EncodedFrame>,
    cancellation: CancellationToken,
) -> Result<(), String> {
    loop {
        tokio::select! {
            _ = cancellation.cancelled() => {
                return writer
                    .close()
                    .await
                    .map_err(|error| format!("failed to close code-mode host connection: {error}"));
            }
            frame = outgoing.recv() => {
                let Some(frame) = frame else {
                    return Err("code-mode host outgoing stream closed".to_string());
                };
                tokio::select! {
                    _ = cancellation.cancelled() => return Ok(()),
                    result = writer.write_frame(frame) => {
                        result.map_err(|error| {
                            format!("failed to write code-mode host message: {error}")
                        })?;
                    }
                }
            }
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        mark_connection_dead(
            &self.alive,
            &self.failure,
            "code-mode host connection closed".to_string(),
        );
        self.cancellation.cancel();
    }
}

impl ConnectionSupervisor {
    async fn run(mut self) {
        let mut owner_exited = false;
        let reason = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => failure_message(&self.failure),
            result = &mut self.driver_task => match result {
                Ok(()) => "code-mode connection driver exited unexpectedly".to_string(),
                Err(err) => format!("code-mode connection driver task failed: {err}"),
            },
            result = &mut self.reader_task => task_failure("reader", result),
            result = &mut self.writer_task => task_failure("writer", result),
            reason = self.owner.wait() => {
                owner_exited = true;
                reason
            }
        };
        mark_connection_dead(&self.alive, &self.failure, reason.clone());
        let _ = self.event_tx.try_send(DriverEvent::Failed(reason));
        self.cancellation.cancel();
        if !owner_exited {
            self.owner.close().await;
        }
    }
}

impl ConnectionOwner {
    async fn wait(&mut self) -> String {
        match self {
            Self::Process(child) => match child.wait().await {
                Ok(status) => format!("code-mode host exited with status {status}"),
                Err(error) => format!("failed waiting for code-mode host: {error}"),
            },
            Self::WebSocket => std::future::pending().await,
        }
    }

    async fn close(&mut self) {
        match self {
            Self::Process(child) => kill_and_reap(child).await,
            Self::WebSocket => {}
        }
    }
}

fn task_failure(
    task_name: &str,
    result: Result<Result<(), String>, tokio::task::JoinError>,
) -> String {
    match result {
        Ok(Ok(())) => format!("code-mode connection {task_name} exited unexpectedly"),
        Ok(Err(err)) => err,
        Err(err) => format!("code-mode connection {task_name} task failed: {err}"),
    }
}

fn mark_connection_dead(
    alive: &AtomicBool,
    failure: &std::sync::Mutex<Option<String>>,
    reason: String,
) {
    alive.store(false, Ordering::Release);
    let mut failure = failure
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if failure.is_none() {
        *failure = Some(reason);
    }
}

fn failure_message(failure: &std::sync::Mutex<Option<String>>) -> String {
    failure
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
        .unwrap_or_else(|| "code-mode host connection closed".to_string())
}

async fn kill_and_reap(child: &mut Child) {
    let _ = child.start_kill();
    let _ = child.wait().await;
}
