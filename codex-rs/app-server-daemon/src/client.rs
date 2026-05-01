use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::InitializeResponse;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::RequestId;
use codex_uds::UnixStream;
use futures::SinkExt;
use futures::StreamExt;
use tokio::time::timeout;
use tokio_tungstenite::client_async;
use tokio_tungstenite::tungstenite::Message;

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const CLIENT_NAME: &str = "codex_app_server_daemon";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeInfo {
    pub(crate) app_server_version: String,
}

pub(crate) async fn probe(socket_path: &Path) -> Result<ProbeInfo> {
    timeout(PROBE_TIMEOUT, probe_inner(socket_path))
        .await
        .with_context(|| {
            format!(
                "timed out probing app-server control socket {}",
                socket_path.display()
            )
        })?
}

async fn probe_inner(socket_path: &Path) -> Result<ProbeInfo> {
    let stream = UnixStream::connect(socket_path)
        .await
        .with_context(|| format!("failed to connect to {}", socket_path.display()))?;
    let (mut websocket, _response) = client_async("ws://localhost/", stream)
        .await
        .with_context(|| format!("failed to upgrade {}", socket_path.display()))?;

    let initialize = JSONRPCMessage::Request(JSONRPCRequest {
        id: RequestId::Integer(1),
        method: "initialize".to_string(),
        params: Some(serde_json::to_value(InitializeParams {
            client_info: ClientInfo {
                name: CLIENT_NAME.to_string(),
                title: Some("Codex App Server Daemon".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: None,
        })?),
        trace: None,
    });
    websocket
        .send(Message::Text(serde_json::to_string(&initialize)?.into()))
        .await
        .context("failed to send initialize request")?;

    let response = loop {
        let frame = websocket
            .next()
            .await
            .ok_or_else(|| anyhow!("app-server closed before initialize response"))??;
        let Message::Text(payload) = frame else {
            continue;
        };
        let message = serde_json::from_str::<JSONRPCMessage>(&payload)?;
        if let JSONRPCMessage::Response(response) = message
            && response.id == RequestId::Integer(1)
        {
            break response;
        }
    };
    let initialize_response = serde_json::from_value::<InitializeResponse>(response.result)?;

    let initialized = JSONRPCMessage::Notification(JSONRPCNotification {
        method: "initialized".to_string(),
        params: None,
    });
    websocket
        .send(Message::Text(serde_json::to_string(&initialized)?.into()))
        .await
        .context("failed to send initialized notification")?;
    websocket.close(None).await.ok();

    Ok(ProbeInfo {
        app_server_version: parse_version_from_user_agent(&initialize_response.user_agent)?,
    })
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

#[cfg(test)]
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
