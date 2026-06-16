use std::process::Stdio;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::connect_async_with_config;
use tracing::debug;
use tracing::warn;

use codex_utils_rustls_provider::ensure_rustls_crypto_provider;

use crate::ExecServerClient;
use crate::ExecServerError;
use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT;
use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_INITIALIZE_TIMEOUT;
use crate::client_api::NoiseRendezvousConnectArgs;
use crate::client_api::NoiseRendezvousConnectBundle;
use crate::client_api::RemoteExecServerConnectArgs;
use crate::client_api::StdioExecServerCommand;
use crate::client_api::StdioExecServerConnectArgs;
use crate::connection::JsonRpcConnection;
use crate::noise_relay::NoiseHarnessConnectionArgs;
use crate::noise_relay::noise_harness_connection_from_websocket;
use crate::noise_relay::noise_relay_websocket_config;
use crate::relay::harness_connection_from_websocket;

const ENVIRONMENT_CLIENT_NAME: &str = "codex-environment";

impl ExecServerClient {
    /// Open the selected transport and run the common JSON-RPC initialization.
    /// Noise connection details are fetched here so reconnects get a fresh URL
    /// and authorization without replacing the harness identity.
    pub(crate) async fn connect_for_transport(
        transport_params: crate::client_api::ExecServerTransportParams,
    ) -> Result<Self, ExecServerError> {
        match transport_params {
            crate::client_api::ExecServerTransportParams::WebSocketUrl {
                websocket_url,
                connect_timeout,
                initialize_timeout,
            } => {
                Self::connect_websocket(RemoteExecServerConnectArgs {
                    websocket_url,
                    client_name: ENVIRONMENT_CLIENT_NAME.to_string(),
                    connect_timeout,
                    initialize_timeout,
                    resume_session_id: None,
                })
                .await
            }
            crate::client_api::ExecServerTransportParams::NoiseRendezvous {
                provider,
                identity,
            } => {
                let bundle = provider.connect_bundle(identity.public_key()).await?;
                Self::connect_noise_rendezvous(NoiseRendezvousConnectArgs {
                    bundle,
                    harness_identity: identity,
                    client_name: ENVIRONMENT_CLIENT_NAME.to_string(),
                    connect_timeout: DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
                    initialize_timeout: DEFAULT_REMOTE_EXEC_SERVER_INITIALIZE_TIMEOUT,
                    resume_session_id: None,
                })
                .await
            }
            crate::client_api::ExecServerTransportParams::StdioCommand {
                command,
                initialize_timeout,
            } => {
                Self::connect_stdio_command(StdioExecServerConnectArgs {
                    command,
                    client_name: ENVIRONMENT_CLIENT_NAME.to_string(),
                    initialize_timeout,
                    resume_session_id: None,
                })
                .await
            }
        }
    }

    pub async fn connect_websocket(
        args: RemoteExecServerConnectArgs,
    ) -> Result<Self, ExecServerError> {
        ensure_rustls_crypto_provider();
        let websocket_url = args.websocket_url.clone();
        let connect_timeout = args.connect_timeout;
        let (stream, _) = timeout(connect_timeout, connect_async(websocket_url.as_str()))
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
        Self::connect(connection, args.into()).await
    }

    /// Connect to one exec-server through an authenticated rendezvous stream.
    /// The executor key is pinned before JSON-RPC starts; the websocket carries
    /// only ciphertext after that.
    pub async fn connect_noise_rendezvous(
        args: NoiseRendezvousConnectArgs,
    ) -> Result<Self, ExecServerError> {
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
        let (stream, _) = timeout(
            connect_timeout,
            connect_async_with_config(
                websocket_url.as_str(),
                Some(noise_relay_websocket_config()),
                /*disable_nagle*/ false,
            ),
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
        Self::connect(
            connection,
            crate::client_api::ExecServerClientConnectOptions {
                client_name,
                initialize_timeout,
                resume_session_id,
            },
        )
        .await
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
