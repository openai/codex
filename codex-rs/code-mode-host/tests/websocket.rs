use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_code_mode_protocol::host::Capability;
use codex_code_mode_protocol::host::CapabilitySet;
use codex_code_mode_protocol::host::ClientHello;
use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::DelegateRequest;
use codex_code_mode_protocol::host::DelegateResponse;
use codex_code_mode_protocol::host::EncodedFrame;
use codex_code_mode_protocol::host::HostHello;
use codex_code_mode_protocol::host::HostRequest;
use codex_code_mode_protocol::host::HostResponse;
use codex_code_mode_protocol::host::HostToClient;
use codex_code_mode_protocol::host::MAX_FRAME_BYTES;
use codex_code_mode_protocol::host::ProtocolVersion;
use codex_code_mode_protocol::host::RequestId;
use codex_code_mode_protocol::host::SessionId;
use codex_code_mode_protocol::host::SupportedProtocolVersions;
use codex_code_mode_protocol::host::WireContentItem;
use codex_code_mode_protocol::host::WireExecuteRequest;
use codex_code_mode_protocol::host::WireResult;
use codex_code_mode_protocol::host::WireRuntimeResponse;
use codex_code_mode_protocol::host::WireToolDefinition;
use codex_code_mode_protocol::host::WireToolKind;
use codex_code_mode_protocol::host::WireToolName;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpStream;
use tokio::process::Child;
use tokio::process::Command;
use tokio::time::timeout;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite::Error as WebSocketError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::http::header::ORIGIN;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

const TEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_WEBSOCKET_FRAME_BYTES: usize = MAX_FRAME_BYTES + std::mem::size_of::<u32>();

struct HostHarness {
    child: Child,
    websocket_url: String,
}

struct HostClient {
    websocket: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl HostHarness {
    async fn start() -> Result<Self> {
        let host_program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?;
        let mut command = Command::new(host_program);
        command
            .args(["--listen", "ws://127.0.0.1:0"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().context("failed to start code-mode host")?;
        let stdout = child
            .stdout
            .take()
            .context("code-mode host stdout was not captured")?;
        let mut lines = BufReader::new(stdout).lines();
        let websocket_url = timeout(TEST_TIMEOUT, lines.next_line())
            .await
            .context("timed out waiting for code-mode host websocket URL")??
            .context("code-mode host exited before publishing its websocket URL")?;
        if !websocket_url.starts_with("ws://127.0.0.1:") {
            anyhow::bail!("unexpected code-mode host websocket URL `{websocket_url}`");
        }

        Ok(Self {
            child,
            websocket_url,
        })
    }

    async fn connect(&self) -> Result<HostClient> {
        let config = WebSocketConfig::default()
            .max_frame_size(Some(MAX_WEBSOCKET_FRAME_BYTES))
            .max_message_size(Some(MAX_WEBSOCKET_FRAME_BYTES));
        let (websocket, _) = timeout(
            TEST_TIMEOUT,
            connect_async_with_config(
                self.websocket_url.as_str(),
                Some(config),
                /*disable_nagle*/ false,
            ),
        )
        .await
        .context("timed out connecting to code-mode host websocket")??;
        Ok(HostClient { websocket })
    }
}

impl HostClient {
    async fn send(&mut self, message: &ClientToHost) -> Result<()> {
        let frame = EncodedFrame::encode(message)?;
        self.send_binary(frame.into_framed_bytes()).await
    }

    async fn send_binary(&mut self, bytes: Vec<u8>) -> Result<()> {
        timeout(
            TEST_TIMEOUT,
            self.websocket.send(Message::Binary(bytes.into())),
        )
        .await
        .context("timed out writing code-mode websocket message")?
        .context("failed to write code-mode websocket message")
    }

    async fn read(&mut self) -> Result<HostToClient> {
        loop {
            let message = timeout(TEST_TIMEOUT, self.websocket.next())
                .await
                .context("timed out waiting for code-mode websocket message")?
                .context("code-mode websocket closed before returning a message")?
                .context("failed to read code-mode websocket message")?;
            match message {
                Message::Binary(bytes) => {
                    return EncodedFrame::decode_framed(&bytes)
                        .context("failed to decode code-mode websocket frame");
                }
                Message::Ping(_) | Message::Pong(_) => {}
                Message::Close(frame) => {
                    anyhow::bail!("code-mode websocket closed unexpectedly: {frame:?}");
                }
                Message::Text(text) => {
                    anyhow::bail!("code-mode host returned a text websocket message: {text}");
                }
                Message::Frame(_) => {
                    anyhow::bail!("code-mode host returned an unexpected raw websocket frame");
                }
            }
        }
    }

    async fn negotiate(&mut self, optional_capabilities: CapabilitySet) -> Result<()> {
        let hello = ClientHello::new(
            SupportedProtocolVersions::try_new([ProtocolVersion::V1])?,
            CapabilitySet::empty(),
            optional_capabilities,
        )?;
        self.send(&ClientToHost::ClientHello(hello)).await?;
        assert_eq!(
            self.read().await?,
            HostToClient::HostHello(HostHello::new(ProtocolVersion::V1, CapabilitySet::empty()))
        );
        Ok(())
    }

    async fn open_session(&mut self, session_id: SessionId) -> Result<()> {
        let id = RequestId::new(/*value*/ 1);
        self.send(&ClientToHost::Request {
            id,
            request: HostRequest::OpenSession {
                session_id: session_id.clone(),
            },
        })
        .await?;
        assert_eq!(
            self.read().await?,
            HostToClient::Response {
                id,
                result: WireResult::Ok {
                    value: HostResponse::SessionReady { session_id },
                },
            }
        );
        Ok(())
    }
}

#[tokio::test]
async fn websocket_listener_serves_readiness_endpoint() -> Result<()> {
    let host = HostHarness::start().await?;
    let address = host
        .websocket_url
        .strip_prefix("ws://")
        .context("code-mode host websocket URL should use ws://")?;

    let response = timeout(TEST_TIMEOUT, async {
        let mut stream = TcpStream::connect(address)
            .await
            .context("failed to connect to code-mode host readiness endpoint")?;
        let request =
            format!("GET /readyz HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
        stream
            .write_all(request.as_bytes())
            .await
            .context("failed to request code-mode host readiness")?;

        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .await
            .context("failed to read code-mode host readiness response")?;
        Ok::<_, anyhow::Error>(response)
    })
    .await
    .context("timed out requesting code-mode host readiness")??;

    let status_line = response
        .lines()
        .next()
        .context("code-mode host readiness response is missing a status line")?;
    assert_eq!(status_line, "HTTP/1.1 200 OK");
    Ok(())
}

#[tokio::test]
async fn websocket_listener_executes_cells_and_forwards_tool_callbacks() -> Result<()> {
    let host = HostHarness::start().await?;
    let mut client = host.connect().await?;
    client.negotiate(CapabilitySet::empty()).await?;

    let session_id = SessionId::new("websocket-session")?;
    client.open_session(session_id.clone()).await?;

    let execute_id = RequestId::new(/*value*/ 2);
    client
        .send(&ClientToHost::Request {
            id: execute_id,
            request: HostRequest::Execute {
                session_id: session_id.clone(),
                request: WireExecuteRequest {
                    tool_call_id: "websocket-call".to_string(),
                    enabled_tools: vec![WireToolDefinition {
                        name: "echo".to_string(),
                        tool_name: WireToolName {
                            name: "echo".to_string(),
                            namespace: None,
                        },
                        description: String::new(),
                        kind: WireToolKind::Function,
                        input_schema: None,
                        output_schema: None,
                    }],
                    source:
                        r#"const result = await tools.echo({ value: "ping" }); text(result.value);"#
                            .to_string(),
                    yield_time_ms: Some(5_000),
                    max_output_tokens: Some(1_000),
                },
            },
        })
        .await?;

    let started = client.read().await?;
    let HostToClient::Response {
        id,
        result:
            WireResult::Ok {
                value: HostResponse::ExecutionStarted { cell_id },
            },
    } = started
    else {
        anyhow::bail!("expected execution-started response, got {started:?}");
    };
    assert_eq!(id, execute_id);

    let callback = client.read().await?;
    let HostToClient::DelegateRequest {
        id: delegate_id,
        session_id: callback_session_id,
        request: DelegateRequest::InvokeTool { invocation },
    } = callback
    else {
        anyhow::bail!("expected tool callback, got {callback:?}");
    };
    assert_eq!(callback_session_id, session_id);
    assert_eq!(invocation.input, Some(json!({ "value": "ping" })));

    client
        .send(&ClientToHost::DelegateResponse {
            id: delegate_id,
            result: WireResult::Ok {
                value: DelegateResponse::ToolResult {
                    result: json!({ "value": "pong" }),
                },
            },
        })
        .await?;

    assert_eq!(
        client.read().await?,
        HostToClient::InitialResponse {
            id: execute_id,
            result: WireResult::Ok {
                value: WireRuntimeResponse::Result {
                    cell_id,
                    content_items: vec![WireContentItem::InputText {
                        text: "pong".to_string(),
                    }],
                    error_text: None,
                },
            },
        }
    );
    Ok(())
}

#[tokio::test]
async fn websocket_listener_accepts_frames_larger_than_default_websocket_limit() -> Result<()> {
    let host = HostHarness::start().await?;
    let mut client = host.connect().await?;
    let capability = Capability::new("x".repeat((16 * 1024 * 1024) + 1))?;

    client
        .negotiate(CapabilitySet::try_new([capability])?)
        .await
}

#[tokio::test]
async fn websocket_listener_keeps_connections_and_session_ids_isolated() -> Result<()> {
    let host = HostHarness::start().await?;
    let mut first = host.connect().await?;
    let mut second = host.connect().await?;
    first.negotiate(CapabilitySet::empty()).await?;
    second.negotiate(CapabilitySet::empty()).await?;

    let session_id = SessionId::new("shared-session-name")?;
    first.open_session(session_id.clone()).await?;
    second.open_session(session_id).await?;
    Ok(())
}

#[tokio::test]
async fn malformed_websocket_frame_does_not_stop_the_listener() -> Result<()> {
    let mut host = HostHarness::start().await?;
    let stderr = host
        .child
        .stderr
        .take()
        .context("code-mode host stderr was not captured")?;
    let mut stderr_lines = BufReader::new(stderr).lines();
    let mut malformed = host.connect().await?;
    malformed.send_binary(vec![1, 0, 0, 0, b'{']).await?;

    let close = timeout(TEST_TIMEOUT, malformed.websocket.next())
        .await
        .context("timed out waiting for malformed websocket connection to close")?;
    if let Some(Ok(message)) = close
        && !matches!(message, Message::Close(_))
    {
        anyhow::bail!("malformed websocket returned an unexpected message: {message:?}");
    }

    let diagnostic = timeout(TEST_TIMEOUT, async {
        loop {
            let line = stderr_lines
                .next_line()
                .await?
                .context("code-mode host exited before reporting the malformed frame")?;
            if line.contains("code-mode host websocket connection failed") {
                return Ok::<_, anyhow::Error>(line);
            }
        }
    })
    .await
    .context("timed out waiting for the malformed websocket diagnostic")??;
    assert!(
        diagnostic.contains("failed to read code-mode client hello"),
        "unexpected malformed websocket diagnostic: {diagnostic}",
    );

    let mut recovered = host.connect().await?;
    recovered.negotiate(CapabilitySet::empty()).await
}

#[tokio::test]
async fn websocket_listener_rejects_browser_origin_handshakes() -> Result<()> {
    let host = HostHarness::start().await?;
    let mut request = host.websocket_url.as_str().into_client_request()?;
    request
        .headers_mut()
        .insert(ORIGIN, HeaderValue::from_static("https://evil.example"));

    let error = match connect_async(request).await {
        Ok(_) => anyhow::bail!("browser-origin websocket handshake should be rejected"),
        Err(error) => error,
    };
    let WebSocketError::Http(response) = error else {
        anyhow::bail!("browser-origin websocket handshake failed unexpectedly: {error}");
    };
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    Ok(())
}
