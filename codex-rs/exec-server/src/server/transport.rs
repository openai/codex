//! Exec-server stdio and WebSocket listeners. Configured authentication gates
//! WebSocket upgrades; unauthenticated listeners remain supported for compatibility.

use crate::noise_channel::NoiseChannelIdentity;
use crate::noise_relay::MAX_NOISE_RELAY_WEBSOCKET_MESSAGE_SIZE;
use crate::noise_relay::admission::HandshakeBudget;
use crate::noise_relay::admission::InboundAdmission;
use crate::relay::run_multiplexed_environment;
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::extract::State;
use axum::extract::ws::Message as AxumMessage;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::HeaderMap;
use axum::http::Request;
use axum::http::StatusCode;
use axum::http::header::ORIGIN;
use axum::middleware;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::any;
use axum::routing::get;
use codex_http_client::HttpClientFactory;
use codex_websocket_auth::WebsocketAuthPolicy;
use codex_websocket_auth::WebsocketAuthSettings;
use codex_websocket_auth::authorize_upgrade;
use codex_websocket_auth::policy_from_settings;
use futures::SinkExt;
use futures::StreamExt;
use std::io::Write as _;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::sync::Semaphore;
use tokio_tungstenite::tungstenite::Message as NoiseMessage;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::info;
use tracing::warn;

use crate::ExecServerRuntimeOptions;
use crate::ExecServerTelemetry;
use crate::connection::JsonRpcConnection;
use crate::server::RequestDispatchMode;
use crate::server::processor::ConnectionProcessor;
use crate::telemetry::ConnectionTransport;

pub const DEFAULT_LISTEN_URL: &str = "ws://127.0.0.1:0";

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ExecServerListenTransport {
    WebSocket(SocketAddr),
    Stdio,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ExecServerListenUrlParseError {
    UnsupportedListenUrl(String),
    InvalidWebSocketListenUrl(String),
}

impl std::fmt::Display for ExecServerListenUrlParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecServerListenUrlParseError::UnsupportedListenUrl(listen_url) => write!(
                f,
                "unsupported --listen URL `{listen_url}`; expected `ws://IP:PORT` or `stdio`"
            ),
            ExecServerListenUrlParseError::InvalidWebSocketListenUrl(listen_url) => write!(
                f,
                "invalid websocket --listen URL `{listen_url}`; expected `ws://IP:PORT`"
            ),
        }
    }
}

impl std::error::Error for ExecServerListenUrlParseError {}

pub(crate) fn parse_listen_url(
    listen_url: &str,
) -> Result<ExecServerListenTransport, ExecServerListenUrlParseError> {
    if matches!(listen_url, "stdio" | "stdio://") {
        return Ok(ExecServerListenTransport::Stdio);
    }

    if let Some(socket_addr) = listen_url.strip_prefix("ws://") {
        return socket_addr
            .parse::<SocketAddr>()
            .map(ExecServerListenTransport::WebSocket)
            .map_err(|_| {
                ExecServerListenUrlParseError::InvalidWebSocketListenUrl(listen_url.to_string())
            });
    }

    Err(ExecServerListenUrlParseError::UnsupportedListenUrl(
        listen_url.to_string(),
    ))
}

pub(crate) async fn run_transport(
    listen_url: &str,
    runtime_paths: ExecServerRuntimeOptions,
    telemetry: ExecServerTelemetry,
    http_client_factory: HttpClientFactory,
    request_dispatch_mode: RequestDispatchMode,
    websocket_auth: WebsocketAuthSettings,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match parse_listen_url(listen_url)? {
        ExecServerListenTransport::WebSocket(bind_address) => {
            run_websocket_listener(
                bind_address,
                runtime_paths,
                telemetry,
                http_client_factory,
                request_dispatch_mode,
                policy_from_settings(&websocket_auth)?,
            )
            .await
        }
        ExecServerListenTransport::Stdio => {
            if websocket_auth.config.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "WebSocket auth requires a WebSocket listener, not stdio",
                )
                .into());
            }
            run_stdio_connection(
                runtime_paths,
                telemetry,
                http_client_factory,
                request_dispatch_mode,
            )
            .await
        }
    }
}

async fn run_stdio_connection(
    runtime_paths: ExecServerRuntimeOptions,
    telemetry: ExecServerTelemetry,
    http_client_factory: HttpClientFactory,
    request_dispatch_mode: RequestDispatchMode,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    run_stdio_connection_with_io(
        io::stdin(),
        io::stdout(),
        runtime_paths,
        telemetry,
        http_client_factory,
        request_dispatch_mode,
    )
    .await
}

async fn run_stdio_connection_with_io<R, W>(
    reader: R,
    writer: W,
    runtime_paths: ExecServerRuntimeOptions,
    telemetry: ExecServerTelemetry,
    http_client_factory: HttpClientFactory,
    request_dispatch_mode: RequestDispatchMode,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let processor = ConnectionProcessor::new_with_telemetry(
        runtime_paths,
        telemetry,
        http_client_factory,
        request_dispatch_mode,
    );
    tracing::info!("codex-exec-server listening on stdio");
    processor
        .run_connection(
            JsonRpcConnection::from_stdio(reader, writer, "exec-server stdio".to_string()),
            ConnectionTransport::Stdio,
        )
        .await;
    // Stdio serves exactly one connection, so detached sessions cannot be resumed.
    processor.shutdown().await;
    Ok(())
}

async fn run_websocket_listener(
    bind_address: SocketAddr,
    runtime_paths: ExecServerRuntimeOptions,
    telemetry: ExecServerTelemetry,
    http_client_factory: HttpClientFactory,
    request_dispatch_mode: RequestDispatchMode,
    auth_policy: WebsocketAuthPolicy,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(bind_address).await?;
    let local_addr = listener.local_addr()?;
    let processor = ConnectionProcessor::new_with_telemetry(
        runtime_paths,
        telemetry,
        http_client_factory,
        request_dispatch_mode,
    );
    info!("codex-exec-server listening on ws://{local_addr}");
    println!("ws://{local_addr}");
    std::io::stdout().flush()?;

    serve_websocket(
        listener,
        ExecServerWebSocketState {
            processor,
            auth_policy,
            noise: None,
        },
    )
    .await?;
    Ok(())
}

const FIRST_NOISE_AUTHORIZATION_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Default)]
pub(crate) struct NoiseListenerControl {
    shutdown: CancellationToken,
    tasks: TaskTracker,
    budget: Arc<HandshakeBudget>,
}

impl NoiseListenerControl {
    pub(crate) fn stop(&self) {
        self.shutdown.cancel();
    }
    pub(crate) async fn cancelled(&self) {
        self.shutdown.cancelled().await;
    }
    pub(crate) async fn wait(&self) {
        self.tasks.close();
        self.tasks.wait().await;
    }
}

#[derive(Clone)]
struct NoiseListener {
    environment_id: String,
    registration_id: String,
    identity: NoiseChannelIdentity,
    validator: crate::remote::RegistryHarnessKeyValidator,
    permits: Arc<Semaphore>,
    control: NoiseListenerControl,
}

pub(crate) async fn run_noise_websocket_listener(
    listener: TcpListener,
    processor: ConnectionProcessor,
    environment_id: String,
    registration_id: String,
    identity: NoiseChannelIdentity,
    validator: crate::remote::RegistryHarnessKeyValidator,
    control: NoiseListenerControl,
) -> std::io::Result<()> {
    serve_websocket(
        listener,
        ExecServerWebSocketState {
            processor,
            auth_policy: WebsocketAuthPolicy::default(),
            noise: Some(NoiseListener {
                environment_id,
                registration_id,
                identity,
                validator,
                permits: Arc::new(Semaphore::new(64)),
                control,
            }),
        },
    )
    .await
}

async fn serve_websocket(
    listener: TcpListener,
    state: ExecServerWebSocketState,
) -> std::io::Result<()> {
    let shutdown = state
        .noise
        .as_ref()
        .map(|noise| noise.control.shutdown.clone());
    let router = Router::new()
        .route("/", any(websocket_upgrade_handler))
        .route("/readyz", get(readiness_handler))
        .layer(middleware::from_fn(reject_requests_with_origin_header))
        .with_state(state);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        match shutdown {
            Some(shutdown) => shutdown.cancelled().await,
            None => std::future::pending::<()>().await,
        }
    })
    .await
}

#[derive(Clone)]
struct ExecServerWebSocketState {
    processor: ConnectionProcessor,
    auth_policy: WebsocketAuthPolicy,
    noise: Option<NoiseListener>,
}

async fn readiness_handler() -> StatusCode {
    StatusCode::OK
}

async fn reject_requests_with_origin_header(
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if request.headers().contains_key(ORIGIN) {
        warn!(
            method = %request.method(),
            uri = %request.uri(),
            "rejecting exec-server websocket listener request with Origin header"
        );
        Err(StatusCode::FORBIDDEN)
    } else {
        Ok(next.run(request).await)
    }
}

async fn websocket_upgrade_handler(
    websocket: WebSocketUpgrade,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    State(state): State<ExecServerWebSocketState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(err) = authorize_upgrade(&headers, &state.auth_policy) {
        return (err.status_code(), err.message()).into_response();
    }
    let (websocket, permit) = if let Some(noise) = &state.noise {
        let Ok(permit) = Arc::clone(&noise.permits).try_acquire_owned() else {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        };
        (
            websocket
                .max_message_size(MAX_NOISE_RELAY_WEBSOCKET_MESSAGE_SIZE)
                .max_frame_size(MAX_NOISE_RELAY_WEBSOCKET_MESSAGE_SIZE),
            Some(permit),
        )
    } else {
        (websocket, None)
    };
    info!(%peer_addr, "exec-server websocket client connected");
    // Register before Axum spawns the upgrade callback, including upgrades still pending.
    let task = state
        .noise
        .as_ref()
        .map(|noise| noise.control.tasks.token());
    let mut response = websocket
        .on_upgrade(move |stream| async move {
            let _task = task;
            let _permit = permit;
            if let Some(noise) = state.noise {
                let stream = stream
                    .with(|message: NoiseMessage| {
                        futures::future::ready(Ok::<_, axum::Error>(match message {
                            NoiseMessage::Binary(v) => AxumMessage::Binary(v),
                            NoiseMessage::Text(v) => AxumMessage::Text(v.as_str().into()),
                            NoiseMessage::Ping(v) => AxumMessage::Ping(v),
                            NoiseMessage::Pong(v) => AxumMessage::Pong(v),
                            NoiseMessage::Close(_) | NoiseMessage::Frame(_) => {
                                AxumMessage::Close(None)
                            }
                        }))
                    })
                    .map(|result| {
                        result.map(|message| match message {
                            AxumMessage::Binary(v) => NoiseMessage::Binary(v),
                            AxumMessage::Text(v) => NoiseMessage::Text(v.as_str().into()),
                            AxumMessage::Ping(v) => NoiseMessage::Ping(v),
                            AxumMessage::Pong(v) => NoiseMessage::Pong(v),
                            AxumMessage::Close(_) => NoiseMessage::Close(None),
                        })
                    });
                let admission = InboundAdmission {
                    budget: Arc::clone(&noise.control.budget),
                    authorized: Arc::new(Notify::new()),
                    shutdown: noise.control.shutdown.child_token(),
                };
                let authorized = Arc::clone(&admission.authorized);
                let shutdown = admission.shutdown.clone();
                let run = run_multiplexed_environment(
                    stream,
                    state.processor,
                    noise.environment_id,
                    noise.registration_id,
                    noise.identity,
                    noise.validator,
                    Some(admission),
                );
                tokio::pin!(run);
                tokio::select! {
                    _ = &mut run => {},
                    _ = authorized.notified() => { run.await; },
                    _ = tokio::time::sleep(FIRST_NOISE_AUTHORIZATION_TIMEOUT) => {
                        shutdown.cancel();
                        run.await;
                    },
                }
            } else {
                state
                    .processor
                    .run_connection(
                        JsonRpcConnection::from_axum_websocket(
                            stream,
                            format!("exec-server websocket {peer_addr}"),
                        ),
                        ConnectionTransport::WebSocket,
                    )
                    .await;
            }
        })
        .into_response();
    if let Some(request_id) = headers.get("x-request-id") {
        response
            .headers_mut()
            .insert("x-request-id", request_id.clone());
    }
    response
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod transport_tests;
