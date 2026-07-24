use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing::debug;
use tracing::warn;

use codex_http_client::HttpClientFactory;
use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
use codex_websocket_client::WebSocketConnector;
use codex_websocket_client::WebSocketTlsMode;

use crate::ExecServerClient;
use crate::ExecServerError;
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
use crate::noise_relay::noise_harness_connection_from_websocket;
use crate::noise_relay::noise_relay_websocket_config;
use crate::relay::harness_connection_from_websocket;
use crate::trace_context::current_trace_context_headers;

const ENVIRONMENT_CLIENT_NAME: &str = "codex-environment";

/// Reopens the transport for one logical exec-server client session.
///
/// URL connections reuse their configured endpoint. Noise connections retain
/// the harness identity but fetch a fresh single-use authorization bundle for
/// every physical connection attempt.
#[derive(Clone)]
pub(crate) enum ExecServerReconnectStrategy {
    WebSocket(RemoteExecServerConnectArgs),
    NoiseRendezvous {
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
    ) -> Result<(JsonRpcConnection, ExecServerClientConnectOptions), ExecServerError> {
        match self {
            Self::WebSocket(args) => {
                let mut args = args.clone();
                args.resume_session_id = Some(session_id.to_string());
                let connection = ExecServerClient::open_websocket_connection(&args).await?;
                Ok((connection, args.into()))
            }
            Self::NoiseRendezvous {
                provider,
                identity,
                client_name,
                connect_timeout,
                initialize_timeout,
                http_client_factory,
            } => {
                let bundle = provider.connect_bundle(identity.public_key()).await?;
                ExecServerClient::open_noise_rendezvous_connection(NoiseRendezvousConnectArgs {
                    bundle,
                    harness_identity: identity.clone(),
                    client_name: client_name.clone(),
                    connect_timeout: *connect_timeout,
                    initialize_timeout: *initialize_timeout,
                    resume_session_id: Some(session_id.to_string()),
                    http_client_factory: http_client_factory.clone(),
                })
                .await
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

        if let Some(readiness) = deferred_readiness {
            readiness
                .await
                .unwrap_or_else(|_| {
                    Err("environment registration ended before completion".to_string())
                })
                .map_err(|message| {
                    ExecServerError::Disconnected(format!("environment unavailable: {message}"))
                })?;
        }

        let (websocket_url, connect_timeout, initialize_timeout) = match transport_params {
            ExecServerTransportParams::Deferred(_) => {
                return Err(ExecServerError::Protocol(
                    "nested deferred exec-server transports are unsupported".to_string(),
                ));
            }
            ExecServerTransportParams::WebSocketUrl {
                websocket_url,
                connect_timeout,
                initialize_timeout,
            } => (websocket_url, connect_timeout, initialize_timeout),
            ExecServerTransportParams::NoiseRendezvous { provider, identity } => {
                let reconnect_strategy = ExecServerReconnectStrategy::NoiseRendezvous {
                    provider: Arc::clone(&provider),
                    identity: identity.clone(),
                    client_name: ENVIRONMENT_CLIENT_NAME.to_string(),
                    connect_timeout: DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
                    initialize_timeout: DEFAULT_REMOTE_EXEC_SERVER_INITIALIZE_TIMEOUT,
                    http_client_factory: http_client_factory.clone(),
                };
                let (connection, options) = Self::open_initial_noise_rendezvous_connection(
                    &provider,
                    &identity,
                    http_client_factory,
                )
                .await?;
                return Self::connect_with_recovery(connection, options, Some(reconnect_strategy))
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
        Self::connect_websocket(RemoteExecServerConnectArgs {
            websocket_url,
            client_name: ENVIRONMENT_CLIENT_NAME.to_string(),
            connect_timeout,
            initialize_timeout,
            resume_session_id: None,
            http_client_factory,
        })
        .await
    }

    async fn open_initial_noise_rendezvous_connection(
        provider: &Arc<dyn NoiseRendezvousConnectProvider>,
        identity: &NoiseChannelIdentity,
        http_client_factory: HttpClientFactory,
    ) -> Result<(JsonRpcConnection, ExecServerClientConnectOptions), ExecServerError> {
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
        let bundle = provider.connect_bundle(identity.public_key()).await?;
        match open_connection(bundle).await {
            Err(error)
                if matches!(
                    &error,
                    ExecServerError::WebSocketConnect { source, .. }
                        if matches!(
                            source,
                            tokio_tungstenite::tungstenite::Error::Http(response)
                                if response.status().as_u16() == 401
                        )
                ) =>
            {
                let bundle = provider.connect_bundle(identity.public_key()).await?;
                open_connection(bundle).await
            }
            result => result,
        }
    }

    pub async fn connect_websocket(
        args: RemoteExecServerConnectArgs,
    ) -> Result<Self, ExecServerError> {
        let connection = Self::open_websocket_connection(&args).await?;
        let options = args.clone().into();
        Self::connect_with_recovery(
            connection,
            options,
            Some(ExecServerReconnectStrategy::WebSocket(args)),
        )
        .await
    }

    pub(crate) async fn open_websocket_connection(
        args: &RemoteExecServerConnectArgs,
    ) -> Result<JsonRpcConnection, ExecServerError> {
        ensure_rustls_crypto_provider();
        let websocket_url = args.websocket_url.clone();
        let connect_timeout = args.connect_timeout;
        let request = websocket_url
            .as_str()
            .into_client_request()
            .map_err(|source| ExecServerError::WebSocketConnect {
                url: websocket_url.clone(),
                source,
            })?;
        let connector = WebSocketConnector::new_with_tls_mode(
            &args.http_client_factory,
            WebSocketTlsMode::TungsteniteDefault,
        )
        .map_err(|error| ExecServerError::WebSocketConfiguration(error.to_string()))?;
        let (stream, _) = timeout(
            connect_timeout,
            connector.connect(
                request,
                tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default(),
            ),
        )
        .await
        .map_err(|_| ExecServerError::WebSocketConnectTimeout {
            url: websocket_url.clone(),
            timeout: connect_timeout,
        })?
        .map_err(|source| ExecServerError::WebSocketConnect {
            url: websocket_url.clone(),
            source,
        })?;

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
        let (connection, options) = Self::open_noise_rendezvous_connection(args).await?;
        Self::connect(connection, options).await
    }

    pub(crate) async fn open_noise_rendezvous_connection(
        args: NoiseRendezvousConnectArgs,
    ) -> Result<(JsonRpcConnection, ExecServerClientConnectOptions), ExecServerError> {
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
        request
            .headers_mut()
            .extend(current_trace_context_headers());
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
        let connection = noise_harness_connection_from_websocket(
            stream,
            NoiseHarnessConnectionArgs {
                connection_label,
                environment_id,
                executor_registration_id,
                identity: harness_identity,
                responder_public_key: executor_public_key,
                harness_key_authorization,
            },
        );
        Ok((
            connection,
            ExecServerClientConnectOptions {
                client_name,
                initialize_timeout,
                resume_session_id,
            },
        ))
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
