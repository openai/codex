//! Browser OAuth before Desktop has loaded policy. Each pipe owns at most one attempt.

use super::ConfigManager;
use super::config_server_login_failed;
use super::startup_error;
use crate::error_code::invalid_params;
use crate::error_code::invalid_request;
use crate::error_code::method_not_found;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::GatewayOAuthCancelResponse;
use codex_app_server_protocol::GatewayOAuthLoginParams;
use codex_app_server_protocol::GatewayOAuthLoginResponse;
use codex_app_server_protocol::GatewayOAuthStatus;
use codex_app_server_protocol::GatewayOAuthTarget;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::StartupGatewayOAuthChangedNotification;
use codex_login::OAuthSessionManager;
use futures::FutureExt;
use futures::future::BoxFuture;
use serde_json::Value;
use serde_json::json;
use std::io;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

// The policy read already reported that sign-in is required. Use the local
// bootstrap settings to resolve the grant without fetching policy again.
async fn discover_session(
    manager: &ConfigManager,
) -> Result<Option<OAuthSessionManager>, JSONRPCErrorError> {
    let bootstrap = manager
        .load_startup_config(std::env::current_dir().ok())
        .await
        .map_err(startup_error)?
        .config;
    codex_cloud_config::managed_config_auth_manager(
        bootstrap.auth_config(),
        bootstrap.config_layer_stack.requirements_toml(),
    )
    .await
    .map_err(|_| config_server_login_failed("Could not initialize configuration server sign-in"))
}

struct Attempt {
    id: RequestId,
    cancel: CancellationToken,
    login: BoxFuture<'static, io::Result<()>>,
}

impl Attempt {
    async fn start(
        id: RequestId,
        session: OAuthSessionManager,
    ) -> Result<(Self, StartupGatewayOAuthChangedNotification), JSONRPCErrorError> {
        let cancel = CancellationToken::new();
        let cancelled = cancel.clone().cancelled_owned();
        let (tx, rx) = oneshot::channel();
        let mut login = async move {
            session
                .login_with_browser(cancelled, |url| {
                    let _ = tx.send(url.to_string());
                })
                .await
        }
        .boxed();
        let auth_url = tokio::select! {
            _ = &mut login => return Err(config_server_login_failed("Could not start browser sign-in")),
            url = rx => url.map_err(|_| config_server_login_failed("Could not start browser sign-in"))?,
        };
        let handoff = StartupGatewayOAuthChangedNotification {
            target: GatewayOAuthTarget::ConfigServer,
            status: GatewayOAuthStatus::Started,
            auth_url,
        };
        Ok((Self { id, cancel, login }, handoff))
    }

    fn reply(self, result: io::Result<()>) -> Value {
        let result = match result {
            Ok(()) => Ok(json!(GatewayOAuthLoginResponse { prompt: None })),
            Err(_) if self.cancel.is_cancelled() => Err(config_server_login_failed(
                "Configuration server sign-in was cancelled",
            )),
            Err(_) => Err(config_server_login_failed(
                "Configuration server sign-in failed",
            )),
        };
        reply(self.id, result)
    }
}

fn reply(id: RequestId, result: Result<Value, JSONRPCErrorError>) -> Value {
    match result {
        Ok(result) => json!(JSONRPCResponse { id, result }),
        Err(error) => json!(JSONRPCError { id, error }),
    }
}

async fn write(output: &mut (impl AsyncWrite + Unpin), message: Value) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(&message)?;
    bytes.push(b'\n');
    output.write_all(&bytes).await?;
    output.flush().await
}

async fn start_login(
    id: RequestId,
    params: GatewayOAuthLoginParams,
    manager: &ConfigManager,
    active: &mut Option<Attempt>,
) -> Result<StartupGatewayOAuthChangedNotification, JSONRPCErrorError> {
    if params.target != Some(GatewayOAuthTarget::ConfigServer) || params.model_provider.is_some() {
        return Err(invalid_params(
            "Startup login requires the configServer target",
        ));
    }
    if params.device_auth {
        return Err(invalid_params(
            "Startup supports browser authentication only",
        ));
    }
    if active.is_some() {
        return Err(invalid_request("Sign-in already in progress"));
    }
    let session = discover_session(manager).await?.ok_or_else(|| {
        config_server_login_failed("Configuration server OAuth is not configured")
    })?;
    let (attempt, handoff) = Attempt::start(id, session).await?;
    *active = Some(attempt);
    Ok(handoff)
}

async fn handle_request(
    line: &str,
    manager: &ConfigManager,
    active: &mut Option<Attempt>,
    output: &mut (impl AsyncWrite + Unpin),
) -> io::Result<()> {
    let raw: JSONRPCRequest = serde_json::from_str(line).map_err(io::Error::other)?;
    let id = raw.id.clone();
    if !matches!(
        raw.method.as_str(),
        "account/gatewayOAuth/login" | "account/gatewayOAuth/cancel"
    ) {
        return write(
            output,
            reply(id, Err(method_not_found("Unknown startup OAuth method"))),
        )
        .await;
    }
    let result = match ClientRequest::try_from(raw) {
        Ok(ClientRequest::GatewayOAuthLogin { params, .. }) => {
            match start_login(id.clone(), params.unwrap_or_default(), manager, active).await {
                Ok(handoff) => {
                    return write(
                        output,
                        json!({"method":"account/gatewayOAuth/changed","params":handoff}),
                    )
                    .await;
                }
                Err(error) => Err(error),
            }
        }
        Ok(ClientRequest::GatewayOAuthCancel { .. }) => {
            if let Some(mut attempt) = active.take() {
                attempt.cancel.cancel();
                let result = (&mut attempt.login).await;
                write(output, attempt.reply(result)).await?;
            }
            Ok(json!(GatewayOAuthCancelResponse {}))
        }
        _ => Err(invalid_params("Invalid startup OAuth parameters")),
    };
    write(output, reply(id, result)).await
}

pub(super) async fn serve(
    manager: ConfigManager,
    input: impl AsyncRead + Unpin,
    mut output: impl AsyncWrite + Unpin,
) -> io::Result<()> {
    let mut requests = BufReader::new(input).lines();
    let mut active: Option<Attempt> = None;
    loop {
        let line = if let Some(attempt) = active.as_mut() {
            tokio::select! {
                biased;
                result = &mut attempt.login => {
                    if let Some(attempt) = active.take() {
                        write(&mut output, attempt.reply(result)).await?;
                    }
                    continue;
                }
                line = requests.next_line() => line,
            }
        } else {
            requests.next_line().await
        }?;
        let Some(line) = line else { return Ok(()) };
        handle_request(&line, &manager, &mut active, &mut output).await?;
    }
}
