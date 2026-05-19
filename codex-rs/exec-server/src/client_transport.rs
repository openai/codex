use std::process::Stdio;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tracing::debug;
use tracing::info;
use tracing::warn;

use codex_utils_rustls_provider::ensure_rustls_crypto_provider;

use crate::ExecServerClient;
use crate::ExecServerError;
use crate::client_api::RemoteExecServerConnectArgs;
use crate::client_api::StdioExecServerCommand;
use crate::client_api::StdioExecServerConnectArgs;
use crate::connection::JsonRpcConnection;
use crate::relay::harness_connection_from_websocket;

const ENVIRONMENT_CLIENT_NAME: &str = "codex-environment";

impl ExecServerClient {
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
        let redacted_websocket_url = url_without_query(&websocket_url);
        let connect_timeout = args.connect_timeout;
        info!(
            websocket_url = %redacted_websocket_url,
            connect_timeout_ms = connect_timeout.as_millis(),
            "connecting exec-server websocket"
        );
        let (stream, _) =
            match timeout(connect_timeout, connect_async(websocket_url.as_str())).await {
                Ok(Ok(websocket)) => {
                    info!(
                        websocket_url = %redacted_websocket_url,
                        "exec-server websocket transport connected"
                    );
                    websocket
                }
                Ok(Err(source)) => {
                    warn!(
                        websocket_url = %redacted_websocket_url,
                        error = %source,
                        "failed to connect exec-server websocket transport"
                    );
                    return Err(ExecServerError::WebSocketConnect {
                        url: websocket_url.clone(),
                        source,
                    });
                }
                Err(_) => {
                    warn!(
                        websocket_url = %redacted_websocket_url,
                        connect_timeout_ms = connect_timeout.as_millis(),
                        "timed out connecting exec-server websocket transport"
                    );
                    return Err(ExecServerError::WebSocketConnectTimeout {
                        url: websocket_url.clone(),
                        timeout: connect_timeout,
                    });
                }
            };

        let connection_label = format!("exec-server websocket {websocket_url}");
        let connection = if is_rendezvous_harness_url(&websocket_url) {
            harness_connection_from_websocket(stream, connection_label)
        } else {
            JsonRpcConnection::from_websocket(stream, connection_label)
        };
        match Self::connect(connection, args.into()).await {
            Ok(client) => {
                info!(
                    websocket_url = %redacted_websocket_url,
                    session_id = ?client.session_id(),
                    "exec-server websocket initialized"
                );
                Ok(client)
            }
            Err(err) => {
                warn!(
                    websocket_url = %redacted_websocket_url,
                    error = %err,
                    "failed to initialize exec-server websocket"
                );
                Err(err)
            }
        }
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

fn url_without_query(url: &str) -> &str {
    url.split_once('?').map_or(url, |(prefix, _)| prefix)
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
