//! Verify that startup configuration controls compression on the public RPC listener.

use super::connection_handling_websocket::DEFAULT_READ_TIMEOUT;
use super::connection_handling_websocket::create_config_toml;
use super::connection_handling_websocket::spawn_websocket_server_with_args;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::RequestId;
use codex_app_server_transport::WEBSOCKET_COMPRESSION_HEADER;
use codex_app_server_transport::ZSTD_STREAM_PROTOCOL;
use codex_app_server_transport::ZstdDecoder;
use codex_app_server_transport::ZstdEncoder;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;

#[tokio::test]
async fn websocket_compression_requires_server_feature() -> Result<()> {
    for enabled in [false, true] {
        let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
        let codex_home = TempDir::new()?;
        create_config_toml(codex_home.path(), &server.uri(), "never")?;
        let args = if enabled {
            vec![
                "-c".to_string(),
                "features.app_server_websocket_compression=true".to_string(),
            ]
        } else {
            Vec::new()
        };
        let (mut process, address) =
            spawn_websocket_server_with_args(codex_home.path(), "ws://127.0.0.1:0", &args).await?;
        let mut request = format!("ws://{address}").into_client_request()?;
        request.headers_mut().insert(
            WEBSOCKET_COMPRESSION_HEADER,
            HeaderValue::from_static(ZSTD_STREAM_PROTOCOL),
        );
        let (mut websocket, response) = connect_async(request).await?;
        assert_eq!(
            response.headers().get(WEBSOCKET_COMPRESSION_HEADER),
            enabled
                .then(|| HeaderValue::from_static(ZSTD_STREAM_PROTOCOL))
                .as_ref()
        );
        let initialize = json!({
            "id": 1,
            "method": "initialize",
            "params": {"clientInfo": {"name": "compression-test", "version": "1"}}
        })
        .to_string();
        let message = if enabled {
            Message::Binary(ZstdEncoder::new()?.encode(&initialize)?.into())
        } else {
            Message::Text(initialize.into())
        };
        websocket.send(message).await?;
        let response = timeout(DEFAULT_READ_TIMEOUT, websocket.next())
            .await?
            .context("server closed before initialize response")??;
        let text = match (enabled, response) {
            (true, Message::Binary(bytes)) => {
                ZstdDecoder::new(/*max_message_size*/ 1024 * 1024)?.decode(&bytes)?
            }
            (false, Message::Text(text)) => text.to_string(),
            _ => bail!("initialize response used unexpected WebSocket message type"),
        };
        let JSONRPCMessage::Response(response) = serde_json::from_str(&text)? else {
            bail!("expected initialize response");
        };
        assert_eq!(response.id, RequestId::Integer(1));
        process.kill().await?;
    }
    Ok(())
}
