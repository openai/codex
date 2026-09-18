use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::OwnedSemaphorePermit;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio::time::timeout_at;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing::debug;
use tracing::warn;

use codex_api::AuthError;
use codex_api::AuthProvider;
use codex_http_client::HttpClientFactory;
use codex_http_client::Request;
use codex_http_client::RequestCompression;
use codex_protocol::shell_environment::scrub_non_inheritable_env_vars;
use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
use codex_websocket_client::WebSocketConnection;
use codex_websocket_client::WebSocketConnector;
use codex_websocket_client::WebSocketTlsMode;
use http::HeaderMap;

use crate::ExecServerClient;
use crate::ExecServerError;
use crate::client::NoiseInitializeContext;
use crate::client::accepted::AcceptedConnectionSource;
use crate::client::is_retryable_registry_error;
use crate::client::registry_recovery_retry_delay;
use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT;
use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_INITIALIZE_TIMEOUT;
use crate::client_api::ExecServerClientConnectOptions;
use crate::client_api::ExecServerTransportParams;
use crate::client_api::NoiseRendezvousConnectArgs;
use crate::client_api::NoiseRendezvousConnectBundle;
use crate::client_api::NoiseRendezvousConnectProvider;
use crate::client_api::RemoteExecServerConnectArgs;
use crate::client_api::StdioExecServerCommand;
use crate::client_api::StdioExecServerConnectArgs;
use crate::connection::JsonRpcConnection;
use crate::noise_channel::NoiseChannelIdentity;
use crate::noise_relay::NoiseHarnessConnectionArgs;
use crate::noise_relay::noise_harness_connection_from_websocket_with_readiness;
use crate::noise_relay::noise_relay_websocket_config;
use crate::relay::harness_connection_from_websocket;
use crate::trace_context::current_rendezvous_headers;

const ENVIRONMENT_CLIENT_NAME: &str = "codex-environment";
const INITIAL_REGISTRY_MAX_RETRIES: u32 = 4;
const INITIAL_REGISTRY_REQUEST_TIMEOUT: Duration = Duration::from_secs(6);
const INITIAL_REGISTRY_OPERATION_TIMEOUT: Duration = Duration::from_secs(14);

pub(crate) async fn connect_websocket_request(
    request: http::Request<()>,
    diagnostic_url: String,
    connector: WebSocketConnector,
    connect_timeout: Duration,
    use_loopback_direct: bool,
) -> Result<WebSocketConnection, ExecServerError> {
    let websocket_config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default();
    timeout(connect_timeout, async {
        if use_loopback_direct {
            connector
                .connect_loopback_direct(request, websocket_config)
                .await
        } else {
            connector.connect(request, websocket_config).await
        }
    })
    .await
    .map_err(|_| ExecServerError::WebSocketConnectTimeout {
        url: diagnostic_url.clone(),
        timeout: connect_timeout,
    })?
    .map(|(websocket, _)| websocket)
    .map_err(|source| ExecServerError::WebSocketConnect {
        url: diagnostic_url,
        source,
    })
}

pub(crate) async fn authenticate_websocket_request(
    request: &mut http::Request<()>,
    auth_provider: &dyn AuthProvider,
) -> Result<(), AuthError> {
    let url = request.uri().to_string();
    let signing_url = if let Some(rest) = url.strip_prefix("wss://") {
        format!("https://{rest}")
    } else if let Some(rest) = url.strip_prefix("ws://") {
        format!("http://{rest}")
    } else {
        url
    };
    let mut auth_request = Request::new(request.method().clone(), signing_url);
    // Intermediaries may rewrite WebSocket and hop-by-hop headers after signing.
    if let Some(host) = request.headers().get(http::header::HOST) {
        auth_request
            .headers
            .insert(http::header::HOST, host.clone());
    }
    let authenticated = auth_provider.apply_auth(auth_request).await?;
    if authenticated.method != *request.method() {
        return Err(AuthError::Build(
            "authentication changed the WebSocket request method".to_string(),
        ));
    }
    if authenticated.body.is_some() || authenticated.compression != RequestCompression::None {
        return Err(AuthError::Build(
            "authentication added a body or compression to the WebSocket request".to_string(),
        ));
    }

    let authenticated_websocket_url = websocket_url_from_authenticated_url(&authenticated.url)?;
    let authenticated_uri = authenticated_websocket_url.parse().map_err(|error| {
        AuthError::Build(format!("invalid authenticated WebSocket URL: {error}"))
    })?;
    let original_host = request.headers().get(http::header::HOST).cloned();
    for (name, value) in &authenticated.headers {
        if is_websocket_handshake_header(name) {
            if name == http::header::HOST && original_host.as_ref() == Some(value) {
                continue;
            }
            return Err(AuthError::Build(format!(
                "authentication changed WebSocket handshake header {name}"
            )));
        }
        request.headers_mut().insert(name, value.clone());
    }
    *request.uri_mut() = authenticated_uri;
    Ok(())
}

fn websocket_url_from_authenticated_url(url: &str) -> Result<String, AuthError> {
    let mut url = url::Url::parse(url)
        .map_err(|error| AuthError::Build(format!("invalid authenticated request URL: {error}")))?;
    let websocket_scheme = match url.scheme() {
        "https" => "wss",
        "http" => "ws",
        scheme => {
            return Err(AuthError::Build(format!(
                "authentication returned unsupported WebSocket URL scheme: {scheme}"
            )));
        }
    };
    url.set_scheme(websocket_scheme).map_err(|_| {
        AuthError::Build("failed to convert authenticated URL to WebSocket scheme".to_string())
    })?;
    Ok(url.into())
}

fn is_websocket_handshake_header(name: &http::header::HeaderName) -> bool {
    name == http::header::HOST
        || name == http::header::CONNECTION
        || name == http::header::UPGRADE
        || name == http::header::CONTENT_LENGTH
        || name == http::header::TRANSFER_ENCODING
        || name.as_str().starts_with("sec-websocket-")
}

/// Everything the recovery loop needs for one connection attempt.
///
/// An attempt may also carry a permit whose lifetime must extend until the
/// attempt finishes.
pub(crate) struct ReconnectAttempt {
    connection: JsonRpcConnection,
    options: ExecServerClientConnectOptions,
    attempt_permit: Option<OwnedSemaphorePermit>,
    noise_context: Option<NoiseInitializeContext>,
}

struct OpenNoiseRendezvousConnection {
    executor_registration_id: String,
    connection: JsonRpcConnection,
    options: ExecServerClientConnectOptions,
    handshake_ready: tokio::sync::oneshot::Receiver<()>,
}

struct ReadyNoiseRendezvousConnection {
    connection: JsonRpcConnection,
    options: ExecServerClientConnectOptions,
    noise_context: NoiseInitializeContext,
}

impl ReconnectAttempt {
    pub(crate) fn new(
        connection: JsonRpcConnection,
        options: ExecServerClientConnectOptions,
    ) -> Self {
        Self {
            connection,
            options,
            attempt_permit: None,
            noise_context: None,
        }
    }

    fn with_noise_context(
        connection: JsonRpcConnection,
        options: ExecServerClientConnectOptions,
        noise_context: NoiseInitializeContext,
    ) -> Self {
        Self {
            connection,
            options,
            attempt_permit: None,
            noise_context: Some(noise_context),
        }
    }

    pub(crate) fn with_attempt_permit(
        connection: JsonRpcConnection,
        options: ExecServerClientConnectOptions,
        attempt_permit: OwnedSemaphorePermit,
    ) -> Self {
        Self {
            connection,
            options,
            attempt_permit: Some(attempt_permit),
            noise_context: None,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        JsonRpcConnection,
        ExecServerClientConnectOptions,
        Option<OwnedSemaphorePermit>,
        Option<NoiseInitializeContext>,
    ) {
        (
            self.connection,
            self.options,
            self.attempt_permit,
            self.noise_context,
        )
    }
}

/// Reopens the transport for one logical exec-server client session.
///
/// URL connections reuse their configured endpoint. Noise connections retain
/// the harness identity but fetch a fresh single-use authorization bundle for
/// every physical connection attempt.
#[derive(Clone)]
pub(crate) enum ExecServerReconnectStrategy {
    Accepted(AcceptedConnectionSource),
    WebSocket {
        args: RemoteExecServerConnectArgs,
        http_headers: HeaderMap,
    },
    NoiseRendezvous {
        // Registration can renew; recovery still pins the executor's Noise identity.
        executor_public_key: crate::NoiseChannelPublicKey,
        provider: Arc<dyn NoiseRendezvousConnectProvider>,
        identity: NoiseChannelIdentity,
        client_name: String,
        connect_timeout: Duration,
        initialize_timeout: Duration,
        http_client_factory: HttpClientFactory,
    },
}

impl ExecServerReconnectStrategy {
    pub(crate) async fn resume(
        &self,
        session_id: &str,
    ) -> Result<ReconnectAttempt, ExecServerError> {
        match self {
            Self::Accepted(source) => source.next_connection(session_id).await,
            Self::WebSocket { args, http_headers } => {
                let mut args = args.clone();
                args.resume_session_id = Some(session_id.to_string());
                let connection =
                    ExecServerClient::open_websocket_connection(&args, http_headers).await?;
                Ok(ReconnectAttempt::new(connection, args.into()))
            }
            Self::NoiseRendezvous {
                executor_public_key,
                provider,
                identity,
                client_name,
                connect_timeout,
                initialize_timeout,
                http_client_factory,
            } => {
                let bundle = provider.connect_bundle(identity.public_key()).await?;
                if &bundle.executor_public_key != executor_public_key {
                    return Err(ExecServerError::Protocol(
                        "executor key changed during session recovery".to_string(),
                    ));
                }
                // An expired rendezvous URL can make the same executor register again.
                // Initialization must still resume the original session on that executor.
                let opened = ExecServerClient::open_noise_rendezvous_connection(
                    NoiseRendezvousConnectArgs {
                        bundle,
                        harness_identity: identity.clone(),
                        client_name: client_name.clone(),
                        connect_timeout: *connect_timeout,
                        initialize_timeout: *initialize_timeout,
                        resume_session_id: Some(session_id.to_string()),
                        http_client_factory: http_client_factory.clone(),
                    },
                )
                .await?;
                let ready = ExecServerClient::finish_noise_rendezvous_connection(opened).await?;
                Ok(ReconnectAttempt::with_noise_context(
                    ready.connection,
                    ready.options,
                    ready.noise_context,
                ))
            }
        }
    }
}

impl ExecServerClient {
    /// Open the selected transport and run the common JSON-RPC initialization.
    /// Noise connection details are fetched here so reconnects get a fresh URL
    /// and authorization without replacing the harness identity.
    pub(crate) async fn connect_for_transport(
        transport_params: ExecServerTransportParams,
        http_client_factory: HttpClientFactory,
    ) -> Result<Self, ExecServerError> {
        let (transport_params, deferred_readiness) = match transport_params {
            ExecServerTransportParams::Deferred(deferred) => {
                (deferred.transport, Some(deferred.readiness))
            }
            transport_params => (transport_params, None),
        };

        if let Some(mut readiness) = deferred_readiness {
            let provisioning_result = readiness
                .wait_for(Option::is_some)
                .await
                .map_err(|_| {
                    ExecServerError::Disconnected(
                        "environment unavailable: environment provisioning ended before completion"
                            .to_string(),
                    )
                })?
                .clone()
                .ok_or_else(|| {
                    ExecServerError::Disconnected(
                        "environment unavailable: provisioning remained pending after completion"
                            .to_string(),
                    )
                })?;
            provisioning_result.map_err(ExecServerError::ProvisioningFailed)?;
        }

        let websocket = match transport_params {
            ExecServerTransportParams::Deferred(_) => {
                return Err(ExecServerError::Protocol(
                    "nested deferred exec-server transports are unsupported".to_string(),
                ));
            }
            ExecServerTransportParams::WebSocketUrl {
                websocket_url,
                connect_timeout,
                initialize_timeout,
                http_headers,
            } => (
                websocket_url,
                connect_timeout,
                initialize_timeout,
                http_headers,
            ),
            ExecServerTransportParams::NoiseRendezvous { provider, identity } => {
                let (ready, executor_public_key) = Self::open_initial_noise_rendezvous_connection(
                    &provider,
                    &identity,
                    http_client_factory.clone(),
                )
                .await?;
                let reconnect_strategy = ExecServerReconnectStrategy::NoiseRendezvous {
                    executor_public_key,
                    provider,
                    identity,
                    client_name: ENVIRONMENT_CLIENT_NAME.to_string(),
                    connect_timeout: DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
                    initialize_timeout: DEFAULT_REMOTE_EXEC_SERVER_INITIALIZE_TIMEOUT,
                    http_client_factory,
                };
                return Self::connect_with_recovery_and_noise_context(
                    ready.connection,
                    ready.options,
                    Some(reconnect_strategy),
                    ready.noise_context,
                )
                .await;
            }
            ExecServerTransportParams::StdioCommand {
                command,
                initialize_timeout,
            } => {
                return Self::connect_stdio_command(StdioExecServerConnectArgs {
                    command,
                    client_name: ENVIRONMENT_CLIENT_NAME.to_string(),
                    initialize_timeout,
                    resume_session_id: None,
                })
                .await;
            }
        };
        let (websocket_url, connect_timeout, initialize_timeout, http_headers) = websocket;
        Self::connect_websocket_with_headers(
            RemoteExecServerConnectArgs {
                websocket_url,
                client_name: ENVIRONMENT_CLIENT_NAME.to_string(),
                connect_timeout,
                initialize_timeout,
                resume_session_id: None,
                http_client_factory,
            },
            http_headers,
        )
        .await
    }

    #[tracing::instrument(name = "codex.exec_server.remote.noise.connect", skip_all)]
    async fn open_initial_noise_rendezvous_connection(
        provider: &Arc<dyn NoiseRendezvousConnectProvider>,
        identity: &NoiseChannelIdentity,
        http_client_factory: HttpClientFactory,
    ) -> Result<(ReadyNoiseRendezvousConnection, crate::NoiseChannelPublicKey), ExecServerError>
    {
        let open_connection = |bundle: NoiseRendezvousConnectBundle| {
            Self::open_noise_rendezvous_connection(NoiseRendezvousConnectArgs {
                bundle,
                harness_identity: identity.clone(),
                client_name: ENVIRONMENT_CLIENT_NAME.to_string(),
                connect_timeout: DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
                initialize_timeout: DEFAULT_REMOTE_EXEC_SERVER_INITIALIZE_TIMEOUT,
                resume_session_id: None,
                http_client_factory: http_client_factory.clone(),
            })
        };
        let mut deadline = Instant::now() + INITIAL_REGISTRY_OPERATION_TIMEOUT;
        let retry_key = uuid::Uuid::new_v4().to_string();
        let mut retries = 0;
        let mut refreshed_unauthorized_bundle = false;
        let connect_bundle = || async {
            timeout(
                INITIAL_REGISTRY_REQUEST_TIMEOUT,
                provider.connect_bundle(identity.public_key()),
            )
            .await
            .unwrap_or_else(|_| {
                Err(ExecServerError::EnvironmentRegistryRequest(
                    codex_http_client::RouteAwareRequestError::Timeout,
                ))
            })
        };
        let mut result = connect_bundle().await;
        loop {
            let bundle = match result {
                Ok(bundle) => bundle,
                Err(error)
                    if is_retryable_registry_error(&error)
                        && retries < INITIAL_REGISTRY_MAX_RETRIES =>
                {
                    // Session resumption owns its separate recovery deadline.
                    let delay = registry_recovery_retry_delay(&retry_key, retries);
                    retries += 1;
                    result = match timeout_at(deadline, async {
                        sleep(delay).await;
                        connect_bundle().await
                    })
                    .await
                    {
                        Ok(result) => result,
                        Err(_) => return Err(error),
                    };
                    continue;
                }
                Err(error) => return Err(error),
            };
            let executor_public_key = bundle.executor_public_key.clone();
            match open_connection(bundle).await {
                Err(error)
                    if !refreshed_unauthorized_bundle
                        && matches!(
                            &error,
                            ExecServerError::WebSocketConnect { source, .. }
                                if matches!(
                                    source,
                                    tokio_tungstenite::tungstenite::Error::Http(response)
                                        if response.status().as_u16() == 401
                                )
                        ) =>
                {
                    refreshed_unauthorized_bundle = true;
                    deadline = Instant::now() + INITIAL_REGISTRY_OPERATION_TIMEOUT;
                    retries = 0;
                    result = connect_bundle().await;
                }
                result => {
                    let opened = result?;
                    let ready = Self::finish_noise_rendezvous_connection(opened).await?;
                    return Ok((ready, executor_public_key));
                }
            }
        }
    }

    pub async fn connect_websocket(
        args: RemoteExecServerConnectArgs,
    ) -> Result<Self, ExecServerError> {
        Self::connect_websocket_with_headers(args, HeaderMap::new()).await
    }

    async fn connect_websocket_with_headers(
        args: RemoteExecServerConnectArgs,
        http_headers: HeaderMap,
    ) -> Result<Self, ExecServerError> {
        let connection = Self::open_websocket_connection(&args, &http_headers).await?;
        let options = args.clone().into();
        Self::connect_with_recovery(
            connection,
            options,
            Some(ExecServerReconnectStrategy::WebSocket { args, http_headers }),
        )
        .await
    }

    pub(crate) async fn open_websocket_connection(
        args: &RemoteExecServerConnectArgs,
        http_headers: &HeaderMap,
    ) -> Result<JsonRpcConnection, ExecServerError> {
        ensure_rustls_crypto_provider();
        let websocket_url = args.websocket_url.clone();
        let connect_timeout = args.connect_timeout;
        let mut request = websocket_url
            .as_str()
            .into_client_request()
            .map_err(|source| ExecServerError::WebSocketConnect {
                url: websocket_url.clone(),
                source,
            })?;
        request.headers_mut().extend(http_headers.clone());
        let connector = WebSocketConnector::new_with_tls_mode(
            &args.http_client_factory,
            WebSocketTlsMode::TungsteniteDefault,
        )
        .map_err(|error| ExecServerError::WebSocketConfiguration(error.to_string()))?;
        let stream = connect_websocket_request(
            request,
            websocket_url.clone(),
            connector,
            connect_timeout,
            !http_headers.is_empty() && websocket_url.starts_with("ws://"),
        )
        .await?;

        let connection_label = format!("exec-server websocket {websocket_url}");
        let connection = if is_rendezvous_harness_url(&websocket_url) {
            harness_connection_from_websocket(stream, connection_label)
        } else {
            JsonRpcConnection::from_websocket(stream, connection_label)
        };
        Ok(connection)
    }

    /// Connect to one exec-server through an authenticated rendezvous stream
    /// using a caller-supplied single-use authorization bundle.
    ///
    /// The executor key is pinned before JSON-RPC starts; the websocket carries
    /// only ciphertext after that. Environment-managed connections use a
    /// retained [`NoiseRendezvousConnectProvider`] so recovery can fetch a fresh
    /// bundle for each reconnect.
    #[tracing::instrument(
        name = "codex.exec_server.remote.harness.connect",
        skip_all,
        fields(
            otel.kind = "client",
            otel.name = "codex.exec_server.remote.harness.connect",
        )
    )]
    pub async fn connect_noise_rendezvous(
        args: NoiseRendezvousConnectArgs,
    ) -> Result<Self, ExecServerError> {
        let opened = Self::open_noise_rendezvous_connection(args).await?;
        let ready = Self::finish_noise_rendezvous_connection(opened).await?;
        Self::connect_with_recovery_and_noise_context(
            ready.connection,
            ready.options,
            /*reconnect_strategy*/ None,
            ready.noise_context,
        )
        .await
    }

    #[tracing::instrument(
        name = "codex.exec_server.remote.noise.websocket_connect",
        skip_all,
        fields(
            otel.kind = "client",
            otel.name = "codex.exec_server.remote.noise.websocket_connect",
            environment_id = %args.bundle.environment_id,
            executor_registration_id = %args.bundle.executor_registration_id,
        )
    )]
    async fn open_noise_rendezvous_connection(
        args: NoiseRendezvousConnectArgs,
    ) -> Result<OpenNoiseRendezvousConnection, ExecServerError> {
        ensure_rustls_crypto_provider();
        // Keep the registry-issued URL, key, and authorization together for this
        // connection attempt.
        let NoiseRendezvousConnectArgs {
            bundle,
            harness_identity,
            client_name,
            connect_timeout,
            initialize_timeout,
            resume_session_id,
            http_client_factory,
        } = args;
        let NoiseRendezvousConnectBundle {
            websocket_url,
            environment_id,
            executor_registration_id,
            executor_public_key,
            harness_key_authorization,
        } = bundle;
        let diagnostic_url = websocket_url
            .split(['?', '#'])
            .next()
            .unwrap_or(websocket_url.as_str())
            .to_string();
        let mut request = websocket_url
            .as_str()
            .into_client_request()
            .map_err(|source| ExecServerError::WebSocketConnect {
                url: diagnostic_url.clone(),
                source,
            })?;
        request.headers_mut().extend(current_rendezvous_headers());
        let (stream, _) = timeout(
            connect_timeout,
            WebSocketConnector::new_with_tls_mode(
                &http_client_factory,
                WebSocketTlsMode::TungsteniteDefault,
            )
            .map_err(|error| ExecServerError::WebSocketConfiguration(error.to_string()))?
            .with_tcp_nodelay()
            .connect(request, noise_relay_websocket_config()),
        )
        .await
        .map_err(|_| ExecServerError::WebSocketConnectTimeout {
            url: diagnostic_url.clone(),
            timeout: connect_timeout,
        })?
        .map_err(|source| ExecServerError::WebSocketConnect {
            url: diagnostic_url.clone(),
            source,
        })?;

        let connection_label = format!("Noise exec-server rendezvous websocket {diagnostic_url}");
        let connection = noise_harness_connection_from_websocket_with_readiness(
            stream,
            NoiseHarnessConnectionArgs {
                connection_label,
                environment_id,
                executor_registration_id: executor_registration_id.clone(),
                identity: harness_identity,
                responder_public_key: executor_public_key,
                harness_key_authorization,
            },
        );
        Ok(OpenNoiseRendezvousConnection {
            executor_registration_id,
            connection: connection.connection,
            options: ExecServerClientConnectOptions {
                client_name,
                initialize_timeout,
                resume_session_id,
            },
            handshake_ready: connection.handshake_ready,
        })
    }

    #[tracing::instrument(
        name = "codex.exec_server.remote.noise.handshake",
        skip_all,
        parent = initialize_span,
        fields(
            otel.kind = "client",
            otel.name = "codex.exec_server.remote.noise.handshake",
        )
    )]
    async fn wait_for_noise_handshake(
        handshake_ready: &mut tokio::sync::oneshot::Receiver<()>,
        deadline: Instant,
        initialize_timeout: Duration,
        initialize_span: &tracing::Span,
    ) -> Result<(), ExecServerError> {
        match timeout_at(deadline, handshake_ready).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(ExecServerError::Disconnected(
                "Noise harness handshake failed before connection became ready".to_string(),
            )),
            Err(_) => Err(ExecServerError::InitializeTimedOut {
                timeout: initialize_timeout,
            }),
        }
    }

    async fn finish_noise_rendezvous_connection(
        mut connection: OpenNoiseRendezvousConnection,
    ) -> Result<ReadyNoiseRendezvousConnection, ExecServerError> {
        // Preserve the legacy initialize request span as the post-WebSocket
        // startup parent while making its two child operations visible.
        let initialize_timeout = connection.options.initialize_timeout;
        let noise_context = NoiseInitializeContext {
            executor_registration_id: connection.executor_registration_id,
            span: tracing::info_span!(
                "codex.exec_server.request",
                otel.kind = "client",
                otel.name = "initialize",
                method = "initialize",
            ),
            timeout_for_error: initialize_timeout,
        };
        let deadline = Instant::now() + initialize_timeout;
        let readiness = Self::wait_for_noise_handshake(
            &mut connection.handshake_ready,
            deadline,
            initialize_timeout,
            &noise_context.span,
        )
        .await;
        if let Err(error) = readiness {
            // Unlike the normal connect path, the connection has not reached
            // RpcClient yet, so its Drop implementation cannot abort the
            // transport task for us.
            connection.connection.transport.terminate();
            for task in &connection.connection.task_handles {
                task.abort();
            }
            return Err(error);
        }
        let mut options = connection.options;
        options.initialize_timeout = deadline.saturating_duration_since(Instant::now());
        Ok(ReadyNoiseRendezvousConnection {
            connection: connection.connection,
            options,
            noise_context,
        })
    }

    pub(crate) async fn connect_stdio_command(
        args: StdioExecServerConnectArgs,
    ) -> Result<Self, ExecServerError> {
        let mut child = stdio_command_process(&args.command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(ExecServerError::Spawn)?;

        let stdin = child.stdin.take().ok_or_else(|| {
            ExecServerError::Protocol("spawned exec-server command has no stdin".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            ExecServerError::Protocol("spawned exec-server command has no stdout".to_string())
        })?;
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                loop {
                    match lines.next_line().await {
                        Ok(Some(line)) => debug!("exec-server stdio stderr: {line}"),
                        Ok(None) => break,
                        Err(err) => {
                            warn!("failed to read exec-server stdio stderr: {err}");
                            break;
                        }
                    }
                }
            });
        }

        Self::connect(
            JsonRpcConnection::from_stdio(stdout, stdin, "exec-server stdio command".to_string())
                .with_child_process(child),
            args.into(),
        )
        .await
    }
}

fn is_rendezvous_harness_url(websocket_url: &str) -> bool {
    let Some((_path, query)) = websocket_url.split_once('?') else {
        return false;
    };
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .any(|(key, value)| key == "role" && value == "harness")
}

fn stdio_command_process(stdio_command: &StdioExecServerCommand) -> Command {
    let mut command = Command::new(&stdio_command.program);
    command.args(&stdio_command.args);
    command.envs(&stdio_command.env);
    scrub_non_inheritable_env_vars(command.as_std_mut());
    if let Some(cwd) = &stdio_command.cwd {
        command.current_dir(cwd);
    }
    #[cfg(unix)]
    command.process_group(0);
    command
}

#[cfg(test)]
#[path = "client_transport_tests.rs"]
mod tests;
