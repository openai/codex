use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::InitializeResponse;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::RequestId;
use codex_uds::UnixStream;
use futures::SinkExt;
use futures::StreamExt;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::client_async;
use tokio_tungstenite::tungstenite::Message;

pub(crate) const CONTROL_SOCKET_RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);
const CLIENT_NAME: &str = "codex_app_server_daemon";
const INITIALIZE_REQUEST_ID: RequestId = RequestId::Integer(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeInfo {
    pub(crate) app_server_version: String,
}

pub(crate) async fn probe(socket_path: &Path) -> Result<ProbeInfo> {
    timeout(CONTROL_SOCKET_RESPONSE_TIMEOUT, probe_inner(socket_path))
        .await
        .with_context(|| {
            format!(
                "timed out probing app-server control socket {}",
                socket_path.display()
            )
        })?
}

async fn probe_inner(socket_path: &Path) -> Result<ProbeInfo> {
    let mut websocket = connect(socket_path).await?;

    let initialize_response = initialize(&mut websocket, /*experimental_api*/ false).await?;
    let initialized = JSONRPCMessage::Notification(JSONRPCNotification {
        method: "initialized".to_string(),
        params: None,
    });
    send_message(&mut websocket, &initialized)
        .await
        .context("failed to send initialized notification")?;
    websocket.close(None).await.ok();

    Ok(ProbeInfo {
        app_server_version: parse_version_from_user_agent(&initialize_response.user_agent)?,
    })
}

pub(crate) async fn connect(socket_path: &Path) -> Result<WebSocketStream<UnixStream>> {
    let stream = UnixStream::connect(socket_path)
        .await
        .with_context(|| format!("failed to connect to {}", socket_path.display()))?;
    let (websocket, _response) = client_async("ws://localhost/", stream)
        .await
        .with_context(|| format!("failed to upgrade {}", socket_path.display()))?;
    Ok(websocket)
}

pub(crate) async fn initialize<S>(
    websocket: &mut WebSocketStream<S>,
    experimental_api: bool,
) -> Result<InitializeResponse>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let initialize = JSONRPCMessage::Request(JSONRPCRequest {
        id: INITIALIZE_REQUEST_ID,
        method: "initialize".to_string(),
        params: Some(serde_json::to_value(InitializeParams {
            client_info: ClientInfo {
                name: CLIENT_NAME.to_string(),
                title: Some("Codex App Server Daemon".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: if experimental_api {
                Some(InitializeCapabilities {
                    experimental_api: true,
                    ..Default::default()
                })
            } else {
                None
            },
        })?),
        trace: None,
    });
    send_message(websocket, &initialize)
        .await
        .context("failed to send initialize request")?;

    let response = loop {
        let message = timeout(CONTROL_SOCKET_RESPONSE_TIMEOUT, read_message(websocket))
            .await
            .context("timed out waiting for initialize response")??;
        if let JSONRPCMessage::Response(response) = message
            && response.id == INITIALIZE_REQUEST_ID
        {
            break response;
        }
    };
    serde_json::from_value::<InitializeResponse>(response.result)
        .context("failed to parse initialize response")
}

pub(crate) async fn send_message<S>(
    websocket: &mut WebSocketStream<S>,
    message: &JSONRPCMessage,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    websocket
        .send(Message::Text(serde_json::to_string(message)?.into()))
        .await?;
    Ok(())
}

pub(crate) async fn read_message<S>(websocket: &mut WebSocketStream<S>) -> Result<JSONRPCMessage>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    loop {
        let frame = websocket
            .next()
            .await
            .ok_or_else(|| anyhow!("app-server closed the control socket"))??;
        let Message::Text(payload) = frame else {
            continue;
        };
        return serde_json::from_str::<JSONRPCMessage>(&payload)
            .context("failed to parse app-server JSON-RPC message");
    }
}

fn parse_version_from_user_agent(user_agent: &str) -> Result<String> {
    let (_originator, rest) = user_agent
        .split_once('/')
        .ok_or_else(|| anyhow!("app-server user-agent omitted version separator"))?;
    let version = rest
        .split_whitespace()
        .next()
        .filter(|version| !version.is_empty())
        .ok_or_else(|| anyhow!("app-server user-agent omitted version"))?;
    Ok(version.to_string())
}

#[cfg(all(test, unix))]
mod tests {
    use pretty_assertions::assert_eq;

    use super::parse_version_from_user_agent;

    #[test]
    fn parses_version_from_codex_user_agent() {
        assert_eq!(
            parse_version_from_user_agent(
                "codex_app_server_daemon/1.2.3 (Linux 6.8.0; x86_64) codex_cli_rs/1.2.3",
            )
            .expect("version"),
            "1.2.3"
        );
    }

    #[test]
    fn rejects_user_agent_without_version() {
        assert!(parse_version_from_user_agent("codex_app_server_daemon").is_err());
    }
}
