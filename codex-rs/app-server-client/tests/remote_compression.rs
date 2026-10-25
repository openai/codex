//! Exercise a negotiated compressed session through the remote client API.

use std::error::Error;

use codex_app_server_client::AppServerEvent;
use codex_app_server_client::RemoteAppServerClient;
use codex_app_server_client::RemoteAppServerConnectArgs;
use codex_app_server_client::RemoteAppServerEndpoint;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_transport::WEBSOCKET_COMPRESSION_HEADER;
use codex_app_server_transport::ZSTD_STREAM_PROTOCOL;
use codex_app_server_transport::ZstdDecoder;
use codex_app_server_transport::ZstdEncoder;
use codex_uds::UnixListener;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio_tungstenite::accept_hdr_async_with_config;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::Request;
use tokio_tungstenite::tungstenite::handshake::server::Response;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

#[tokio::test]
async fn compressed_session_negotiates_fragments_and_retains_history() -> Result<(), Box<dyn Error>>
{
    const FRAME_LIMIT: usize = 16;
    let dir = TempDir::new()?;
    let socket_path = AbsolutePathBuf::from_absolute_path(dir.path().join("codex.sock"))?;
    let mut listener = UnixListener::bind(socket_path.as_path()).await?;
    let server = async {
        let stream = listener.accept().await?;
        let mut websocket = accept_hdr_async_with_config(
            stream,
            |request: &Request, mut response: Response| {
                assert_eq!(
                    request.headers()[WEBSOCKET_COMPRESSION_HEADER],
                    ZSTD_STREAM_PROTOCOL
                );
                response.headers_mut().insert(
                    WEBSOCKET_COMPRESSION_HEADER,
                    HeaderValue::from_static(ZSTD_STREAM_PROTOCOL),
                );
                response.headers_mut().insert(
                    "x-codex-websocket-max-unfragmented-message-bytes",
                    HeaderValue::from(FRAME_LIMIT),
                );
                Ok(response)
            },
            Some(WebSocketConfig::default().max_frame_size(Some(FRAME_LIMIT))),
        )
        .await?;
        let mut decoder = ZstdDecoder::new(/*max_message_size*/ 1024)?;
        let mut encoder = ZstdEncoder::new()?;
        for expected_method in ["initialize", "initialized", "echo"] {
            let Message::Binary(bytes) = websocket.next().await.ok_or("connection closed")?? else {
                return Err("expected compressed message".into());
            };
            if expected_method == "initialize" {
                assert!(
                    bytes.len() > FRAME_LIMIT,
                    "initialize must require fragmentation"
                );
            }
            let request: Value = serde_json::from_str(&decoder.decode(&bytes)?)?;
            assert_eq!(request["method"], expected_method);
            if expected_method != "initialized" {
                let response = json!({"id": request["id"], "result": {"ok": true}}).to_string();
                websocket
                    .send(Message::Binary(encoder.encode(&response)?.into()))
                    .await?;
            }
        }
        websocket.send(Message::Text("{}".into())).await?;
        Ok::<(), Box<dyn Error>>(())
    };
    let client = async {
        let mut client = RemoteAppServerClient::connect(RemoteAppServerConnectArgs {
            endpoint: RemoteAppServerEndpoint::UnixSocket { socket_path },
            client_name: "compression-test".to_string(),
            client_version: "0.0.0".to_string(),
            experimental_api: false,
            mcp_server_openai_form_elicitation: false,
            opt_out_notification_methods: Vec::new(),
            channel_capacity: 8,
        })
        .await?;
        assert_eq!(
            client
                .request_handle()
                .request_json_rpc(JSONRPCRequest {
                    id: RequestId::Integer(1),
                    method: "echo".to_string(),
                    params: None,
                    trace: None,
                })
                .await?,
            Ok(json!({"ok": true}))
        );
        let Some(AppServerEvent::Disconnected { message }) = client.next_event().await else {
            return Err("expected disconnect after uncompressed message".into());
        };
        assert!(message.contains("expected compressed binary"));
        client.shutdown().await?;
        Ok::<(), Box<dyn Error>>(())
    };
    tokio::try_join!(server, client)?;
    Ok(())
}
