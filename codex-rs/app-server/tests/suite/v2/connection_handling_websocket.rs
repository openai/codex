use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use futures::SinkExt;
use futures::StreamExt;
use hmac::Hmac;
use hmac::Mac;
use reqwest::StatusCode;
use serde_json::json;
use sha2::Sha256;
use std::net::SocketAddr;
use std::path::Path;
use std::process::Stdio;
use tempfile::TempDir;
use time::OffsetDateTime;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::Command;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message as WebSocketMessage;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::http::header::AUTHORIZATION;
use tokio_tungstenite::tungstenite::http::header::ORIGIN;

pub(super) const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) type WsClient = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
type HmacSha256 = Hmac<Sha256>;

#[tokio::test]
async fn websocket_transport_routes_per_connection_handshake_and_responses() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;

    let (mut process, bind_addr) = spawn_websocket_server(codex_home.path()).await?;

    let mut ws1 = connect_websocket(bind_addr).await?;
    let mut ws2 = connect_websocket(bind_addr).await?;

    send_initialize_request(&mut ws1, /*id*/ 1, "ws_client_one").await?;
    let first_init = read_response_for_id(&mut ws1, /*id*/ 1).await?;
    assert_eq!(first_init.id, RequestId::Integer(1));

    // Initialize responses are request-scoped and must not leak to other
    // connections.
    assert_no_message(&mut ws2, Duration::from_millis(250)).await?;

    send_config_read_request(&mut ws2, /*id*/ 2).await?;
    let not_initialized = read_error_for_id(&mut ws2, /*id*/ 2).await?;
    assert_eq!(not_initialized.error.message, "Not initialized");

    send_initialize_request(&mut ws2, /*id*/ 3, "ws_client_two").await?;
    let second_init = read_response_for_id(&mut ws2, /*id*/ 3).await?;
    assert_eq!(second_init.id, RequestId::Integer(3));

    // Same request-id on different connections must route independently.
    send_config_read_request(&mut ws1, /*id*/ 77).await?;
    send_config_read_request(&mut ws2, /*id*/ 77).await?;
    let ws1_config = read_response_for_id(&mut ws1, /*id*/ 77).await?;
    let ws2_config = read_response_for_id(&mut ws2, /*id*/ 77).await?;

    assert_eq!(ws1_config.id, RequestId::Integer(77));
    assert_eq!(ws2_config.id, RequestId::Integer(77));
    assert!(ws1_config.result.get("config").is_some());
    assert!(ws2_config.result.get("config").is_some());

    process
        .kill()
        .await
        .context("failed to stop websocket app-server process")?;
    Ok(())
}

#[tokio::test]
async fn websocket_transport_serves_health_endpoints_on_same_listener() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;

    let (mut process, bind_addr) = spawn_websocket_server(codex_home.path()).await?;
    let client = reqwest::Client::new();

    let readyz = http_get(&client, bind_addr, "/readyz").await?;
    assert_eq!(readyz.status(), StatusCode::OK);

    let healthz = http_get(&client, bind_addr, "/healthz").await?;
    assert_eq!(healthz.status(), StatusCode::OK);

    let mut ws = connect_websocket(bind_addr).await?;
    send_initialize_request(&mut ws, /*id*/ 1, "ws_health_client").await?;
    let init = read_response_for_id(&mut ws, /*id*/ 1).await?;
    assert_eq!(init.id, RequestId::Integer(1));

    process
        .kill()
        .await
        .context("failed to stop websocket app-server process")?;
    Ok(())
}

#[tokio::test]
async fn websocket_transport_rejects_browser_origin_without_auth() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;

    let (mut process, bind_addr) = spawn_websocket_server(codex_home.path()).await?;

    let mut ws = connect_websocket(bind_addr).await?;
    send_initialize_request(&mut ws, /*id*/ 1, "ws_loopback_client").await?;
    let init = read_response_for_id(&mut ws, /*id*/ 1).await?;
    assert_eq!(init.id, RequestId::Integer(1));
    drop(ws);

    assert_websocket_connect_rejected_with_headers(
        bind_addr,
        /*bearer_token*/ None,
        Some("https://evil.example"),
        StatusCode::FORBIDDEN,
    )
    .await?;

    process
        .kill()
        .await
        .context("failed to stop websocket app-server process")?;
    Ok(())
}

#[tokio::test]
async fn websocket_transport_rejects_missing_and_invalid_capability_tokens() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    let token_file = codex_home.path().join("app-server-token");
    std::fs::write(&token_file, "super-secret-token\n")?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let auth_args = vec![
        "--ws-auth".to_string(),
        "capability-token".to_string(),
        "--ws-token-file".to_string(),
        token_file.display().to_string(),
    ];

    let (mut process, bind_addr) =
        spawn_websocket_server_with_args(codex_home.path(), "ws://127.0.0.1:0", &auth_args).await?;

    assert_websocket_connect_rejected(bind_addr, /*bearer_token*/ None).await?;
    assert_websocket_connect_rejected(bind_addr, Some("wrong-token")).await?;

    let mut ws = connect_websocket_with_bearer(bind_addr, Some("super-secret-token")).await?;
    send_initialize_request(&mut ws, /*id*/ 1, "ws_auth_client").await?;
    let init = read_response_for_id(&mut ws, /*id*/ 1).await?;
    assert_eq!(init.id, RequestId::Integer(1));

    process
        .kill()
        .await
        .context("failed to stop websocket app-server process")?;
    Ok(())
}

#[tokio::test]
async fn websocket_transport_verifies_signed_short_lived_bearer_tokens() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    let shared_secret_file = codex_home.path().join("app-server-signing-secret");
    let shared_secret = "0123456789abcdef0123456789abcdef";
    std::fs::write(&shared_secret_file, format!("{shared_secret}\n"))?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let auth_args = vec![
        "--ws-auth".to_string(),
        "signed-bearer-token".to_string(),
        "--ws-shared-secret-file".to_string(),
        shared_secret_file.display().to_string(),
        "--ws-issuer".to_string(),
        "codex-enroller".to_string(),
        "--ws-audience".to_string(),
        "codex-app-server".to_string(),
        "--ws-max-clock-skew-seconds".to_string(),
        "1".to_string(),
    ];

    let (mut process, bind_addr) =
        spawn_websocket_server_with_args(codex_home.path(), "ws://127.0.0.1:0", &auth_args).await?;
    let expired_token = signed_bearer_token(
        shared_secret.as_bytes(),
        json!({
            "exp": OffsetDateTime::now_utc().unix_timestamp() - 30,
            "iss": "codex-enroller",
            "aud": "codex-app-server",
        }),
    )?;
    assert_websocket_connect_rejected(bind_addr, Some(expired_token.as_str())).await?;

    let malformed_token = "not-a-jwt";
    assert_websocket_connect_rejected(bind_addr, Some(malformed_token)).await?;

    let not_yet_valid_token = signed_bearer_token(
        shared_secret.as_bytes(),
        json!({
            "exp": OffsetDateTime::now_utc().unix_timestamp() + 60,
            "nbf": OffsetDateTime::now_utc().unix_timestamp() + 30,
            "iss": "codex-enroller",
            "aud": "codex-app-server",
        }),
    )?;
    assert_websocket_connect_rejected(bind_addr, Some(not_yet_valid_token.as_str())).await?;

    let wrong_issuer_token = signed_bearer_token(
        shared_secret.as_bytes(),
        json!({
            "exp": OffsetDateTime::now_utc().unix_timestamp() + 60,
            "iss": "someone-else",
            "aud": "codex-app-server",
        }),
    )?;
    assert_websocket_connect_rejected(bind_addr, Some(wrong_issuer_token.as_str())).await?;

    let wrong_audience_token = signed_bearer_token(
        shared_secret.as_bytes(),
        json!({
            "exp": OffsetDateTime::now_utc().unix_timestamp() + 60,
            "iss": "codex-enroller",
            "aud": "wrong-audience",
        }),
    )?;
    assert_websocket_connect_rejected(bind_addr, Some(wrong_audience_token.as_str())).await?;

    let wrong_signature_token = signed_bearer_token(
        b"fedcba9876543210fedcba9876543210",
        json!({
            "exp": OffsetDateTime::now_utc().unix_timestamp() + 60,
            "iss": "codex-enroller",
            "aud": "codex-app-server",
        }),
    )?;
    assert_websocket_connect_rejected(bind_addr, Some(wrong_signature_token.as_str())).await?;

    let valid_token = signed_bearer_token(
        shared_secret.as_bytes(),
        json!({
            "exp": OffsetDateTime::now_utc().unix_timestamp() + 60,
            "iss": "codex-enroller",
            "aud": "codex-app-server",
        }),
    )?;
    let mut ws = connect_websocket_with_bearer(bind_addr, Some(valid_token.as_str())).await?;
    send_initialize_request(&mut ws, /*id*/ 1, "ws_signed_auth_client").await?;
    let init = read_response_for_id(&mut ws, /*id*/ 1).await?;
    assert_eq!(init.id, RequestId::Integer(1));

    process
        .kill()
        .await
        .context("failed to stop websocket app-server process")?;
    Ok(())
}

#[tokio::test]
async fn websocket_transport_rejects_short_signed_bearer_secret_configuration() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    let shared_secret_file = codex_home.path().join("app-server-signing-secret");
    std::fs::write(&shared_secret_file, "too-short\n")?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;

    let output = run_websocket_server_to_completion_with_args(
        codex_home.path(),
        "ws://127.0.0.1:0",
        &[
            "--ws-auth".to_string(),
            "signed-bearer-token".to_string(),
            "--ws-shared-secret-file".to_string(),
            shared_secret_file.display().to_string(),
        ],
    )
    .await?;
    assert!(
        !output.status.success(),
        "short shared secret should fail websocket server startup"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be valid utf-8")?;
    assert!(
        stderr.contains("must be at least 32 bytes"),
        "unexpected stderr: {stderr}"
    );

    Ok(())
}

#[tokio::test]
async fn websocket_transport_allows_unauthenticated_non_loopback_startup_by_default() -> Result<()>
{
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;

    let (mut process, bind_addr) =
        spawn_websocket_server_with_args(codex_home.path(), "ws://0.0.0.0:0", &[]).await?;

    let mut ws = connect_websocket(bind_addr).await?;
    send_initialize_request(&mut ws, /*id*/ 1, "ws_non_loopback_default_client").await?;
    let init = read_response_for_id(&mut ws, /*id*/ 1).await?;
    assert_eq!(init.id, RequestId::Integer(1));

    process
        .kill()
        .await
        .context("failed to stop websocket app-server process")?;

    Ok(())
}

#[tokio::test]
async fn websocket_disconnect_keeps_last_subscribed_thread_loaded_until_idle_timeout() -> Result<()>
{
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;

    let (mut process, bind_addr) = spawn_websocket_server(codex_home.path()).await?;

    let mut ws1 = connect_websocket(bind_addr).await?;
    send_initialize_request(&mut ws1, /*id*/ 1, "ws_thread_owner").await?;
    read_response_for_id(&mut ws1, /*id*/ 1).await?;

    let thread_id = start_thread(&mut ws1, /*id*/ 2).await?;
    assert_loaded_threads(&mut ws1, /*id*/ 3, &[thread_id.as_str()]).await?;

    ws1.close(None).await.context("failed to close websocket")?;
    drop(ws1);

    let mut ws2 = connect_websocket(bind_addr).await?;
    send_initialize_request(&mut ws2, /*id*/ 4, "ws_reconnect_client").await?;
    read_response_for_id(&mut ws2, /*id*/ 4).await?;

    wait_for_loaded_threads(&mut ws2, /*first_id*/ 5, &[thread_id.as_str()]).await?;

    process
        .kill()
        .await
        .context("failed to stop websocket app-server process")?;
    Ok(())
}

pub(super) async fn spawn_websocket_server(codex_home: &Path) -> Result<(Child, SocketAddr)> {
    spawn_websocket_server_with_args(codex_home, "ws://127.0.0.1:0", &[]).await
}

pub(super) async fn spawn_websocket_server_with_args(
    codex_home: &Path,
    listen_url: &str,
    extra_args: &[String],
) -> Result<(Child, SocketAddr)> {
    let program = codex_utils_cargo_bin::cargo_bin("codex-app-server")
        .context("should find app-server binary")?;
    let mut cmd = Command::new(program);
    cmd.arg("--listen")
        .arg(listen_url)
        .args(extra_args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .env("CODEX_HOME", codex_home)
        .env("RUST_LOG", "debug");
    let mut process = cmd
        .kill_on_drop(true)
        .spawn()
        .context("failed to spawn websocket app-server process")?;

    let stderr = process
        .stderr
        .take()
        .context("failed to capture websocket app-server stderr")?;
    let mut stderr_reader = BufReader::new(stderr).lines();
    let deadline = Instant::now() + Duration::from_secs(10);
    let bind_addr = loop {
        let line = timeout(
            deadline.saturating_duration_since(Instant::now()),
            stderr_reader.next_line(),
        )
        .await
        .context("timed out waiting for websocket app-server to report bound websocket address")?
        .context("failed to read websocket app-server stderr")?
        .context("websocket app-server exited before reporting bound websocket address")?;
        eprintln!("[websocket app-server stderr] {line}");

        let stripped_line = {
            let mut stripped = String::with_capacity(line.len());
            let mut chars = line.chars().peekable();
            while let Some(ch) = chars.next() {
                if ch == '\u{1b}' && matches!(chars.peek(), Some(&'[')) {
                    chars.next();
                    for next in chars.by_ref() {
                        if ('@'..='~').contains(&next) {
                            break;
                        }
                    }
                    continue;
                }
                stripped.push(ch);
            }
            stripped
        };

        if let Some(bind_addr) = stripped_line
            .split_whitespace()
            .find_map(|token| token.strip_prefix("ws://"))
            .and_then(|addr| addr.parse::<SocketAddr>().ok())
        {
            break bind_addr;
        }
    };

    tokio::spawn(async move {
        while let Ok(Some(line)) = stderr_reader.next_line().await {
            eprintln!("[websocket app-server stderr] {line}");
        }
    });

    Ok((process, bind_addr))
}

pub(super) async fn connect_websocket(bind_addr: SocketAddr) -> Result<WsClient> {
    connect_websocket_with_bearer(bind_addr, /*bearer_token*/ None).await
}

pub(super) async fn connect_websocket_with_bearer(
    bind_addr: SocketAddr,
    bearer_token: Option<&str>,
) -> Result<WsClient> {
    let url = format!("ws://{}", connectable_bind_addr(bind_addr));
    let request = websocket_request(url.as_str(), bearer_token, /*origin*/ None)?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match connect_async(request.clone()).await {
            Ok((stream, _response)) => return Ok(stream),
            Err(err) => {
                if Instant::now() >= deadline {
                    bail!("failed to connect websocket to {url}: {err}");
                }
                sleep(Duration::from_millis(50)).await;
            }
        }
    }
}

async fn assert_websocket_connect_rejected(
    bind_addr: SocketAddr,
    bearer_token: Option<&str>,
) -> Result<()> {
    assert_websocket_connect_rejected_with_headers(
        bind_addr,
        bearer_token,
        /*origin*/ None,
        StatusCode::UNAUTHORIZED,
    )
    .await
}

async fn assert_websocket_connect_rejected_with_headers(
    bind_addr: SocketAddr,
    bearer_token: Option<&str>,
    origin: Option<&str>,
    expected_status: StatusCode,
) -> Result<()> {
    let url = format!("ws://{}", connectable_bind_addr(bind_addr));
    let request = websocket_request(url.as_str(), bearer_token, origin)?;

    match connect_async(request).await {
        Ok((_stream, response)) => {
            bail!(
                "expected websocket handshake rejection, got {}",
                response.status()
            )
        }
        Err(WsError::Http(response)) => {
            assert_eq!(response.status(), expected_status);
            Ok(())
        }
        Err(err) => bail!("expected http rejection during websocket handshake: {err}"),
    }
}

async fn run_websocket_server_to_completion_with_args(
    codex_home: &Path,
    listen_url: &str,
    extra_args: &[String],
) -> Result<std::process::Output> {
    let program = codex_utils_cargo_bin::cargo_bin("codex-app-server")
        .context("should find app-server binary")?;
    let mut cmd = Command::new(program);
    cmd.arg("--listen")
        .arg(listen_url)
        .args(extra_args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .env("CODEX_HOME", codex_home)
        .env("RUST_LOG", "debug");
    timeout(Duration::from_secs(10), cmd.output())
        .await
        .context("timed out waiting for websocket app-server to exit")?
        .context("failed to run websocket app-server")
}

async fn http_get(
    client: &reqwest::Client,
    bind_addr: SocketAddr,
    path: &str,
) -> Result<reqwest::Response> {
    let connectable_bind_addr = connectable_bind_addr(bind_addr);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match client
            .get(format!("http://{connectable_bind_addr}{path}"))
            .send()
            .await
            .with_context(|| format!("failed to GET http://{connectable_bind_addr}{path}"))
        {
            Ok(response) => return Ok(response),
            Err(err) => {
                if Instant::now() >= deadline {
                    bail!("failed to GET http://{connectable_bind_addr}{path}: {err}");
                }
                sleep(Duration::from_millis(50)).await;
            }
        }
    }
}

fn websocket_request(
    url: &str,
    bearer_token: Option<&str>,
    origin: Option<&str>,
) -> Result<tokio_tungstenite::tungstenite::http::Request<()>> {
    let mut request = url
        .into_client_request()
        .context("failed to create websocket request")?;
    if let Some(bearer_token) = bearer_token {
        request.headers_mut().insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {bearer_token}"))
                .context("invalid bearer token header")?,
        );
    }
    if let Some(origin) = origin {
        request.headers_mut().insert(
            ORIGIN,
            HeaderValue::from_str(origin).context("invalid origin header")?,
        );
    }
    Ok(request)
}

pub(super) async fn send_initialize_request(
    stream: &mut WsClient,
    id: i64,
    client_name: &str,
) -> Result<()> {
    let params = InitializeParams {
        client_info: ClientInfo {
            name: client_name.to_string(),
            title: Some("WebSocket Test Client".to_string()),
            version: "0.1.0".to_string(),
        },
        capabilities: None,
    };
    send_request(
        stream,
        "initialize",
        id,
        Some(serde_json::to_value(params)?),
    )
    .await
}

async fn start_thread(stream: &mut WsClient, id: i64) -> Result<String> {
    send_request(
        stream,
        "thread/start",
        id,
        Some(serde_json::to_value(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })?),
    )
    .await?;
    let response = read_response_for_id(stream, id).await?;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(response)?;
    Ok(thread.id)
}

async fn assert_loaded_threads(stream: &mut WsClient, id: i64, expected: &[&str]) -> Result<()> {
    let response = request_loaded_threads(stream, id).await?;
    let mut actual = response.data;
    actual.sort();
    let mut expected = expected
        .iter()
        .map(|thread_id| (*thread_id).to_string())
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(actual, expected);
    assert_eq!(response.next_cursor, None);
    Ok(())
}

async fn wait_for_loaded_threads(
    stream: &mut WsClient,
    first_id: i64,
    expected: &[&str],
) -> Result<()> {
    let mut next_id = first_id;
    let expected = expected
        .iter()
        .map(|thread_id| (*thread_id).to_string())
        .collect::<Vec<_>>();
    timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let response = request_loaded_threads(stream, next_id).await?;
            next_id += 1;
            let mut actual = response.data;
            actual.sort();
            if actual == expected {
                return Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .context("timed out waiting for loaded thread list")??;
    Ok(())
}

async fn request_loaded_threads(
    stream: &mut WsClient,
    id: i64,
) -> Result<ThreadLoadedListResponse> {
    send_request(
        stream,
        "thread/loaded/list",
        id,
        Some(serde_json::to_value(ThreadLoadedListParams::default())?),
    )
    .await?;
    let response = read_response_for_id(stream, id).await?;
    to_response::<ThreadLoadedListResponse>(response)
}

async fn send_config_read_request(stream: &mut WsClient, id: i64) -> Result<()> {
    send_request(
        stream,
        "config/read",
        id,
        Some(json!({ "includeLayers": false })),
    )
    .await
}

pub(super) async fn send_request(
    stream: &mut WsClient,
    method: &str,
    id: i64,
    params: Option<serde_json::Value>,
) -> Result<()> {
    let message = JSONRPCMessage::Request(JSONRPCRequest {
        id: RequestId::Integer(id),
        method: method.to_string(),
        params,
        trace: None,
    });
    send_jsonrpc(stream, message).await
}

async fn send_jsonrpc(stream: &mut WsClient, message: JSONRPCMessage) -> Result<()> {
    let payload = serde_json::to_string(&message)?;
    stream
        .send(WebSocketMessage::Text(payload.into()))
        .await
        .context("failed to send websocket frame")
}

pub(super) async fn read_response_for_id(
    stream: &mut WsClient,
    id: i64,
) -> Result<JSONRPCResponse> {
    let target_id = RequestId::Integer(id);
    loop {
        let message = read_jsonrpc_message(stream).await?;
        if let JSONRPCMessage::Response(response) = message
            && response.id == target_id
        {
            return Ok(response);
        }
    }
}

pub(super) async fn read_notification_for_method(
    stream: &mut WsClient,
    method: &str,
) -> Result<JSONRPCNotification> {
    loop {
        let message = read_jsonrpc_message(stream).await?;
        if let JSONRPCMessage::Notification(notification) = message
            && notification.method == method
        {
            return Ok(notification);
        }
    }
}

pub(super) async fn read_response_and_notification_for_method(
    stream: &mut WsClient,
    id: i64,
    method: &str,
) -> Result<(JSONRPCResponse, JSONRPCNotification)> {
    let target_id = RequestId::Integer(id);
    let mut response = None;
    let mut notification = None;

    while response.is_none() || notification.is_none() {
        let message = read_jsonrpc_message(stream).await?;
        match message {
            JSONRPCMessage::Response(candidate) if candidate.id == target_id => {
                response = Some(candidate);
            }
            JSONRPCMessage::Notification(candidate) if candidate.method == method => {
                if notification.replace(candidate).is_some() {
                    bail!(
                        "received duplicate notification for method `{method}` before completing paired read"
                    );
                }
            }
            _ => {}
        }
    }

    let Some(response) = response else {
        bail!("response must be set before returning");
    };
    let Some(notification) = notification else {
        bail!("notification must be set before returning");
    };

    Ok((response, notification))
}

async fn read_error_for_id(stream: &mut WsClient, id: i64) -> Result<JSONRPCError> {
    let target_id = RequestId::Integer(id);
    loop {
        let message = read_jsonrpc_message(stream).await?;
        if let JSONRPCMessage::Error(err) = message
            && err.id == target_id
        {
            return Ok(err);
        }
    }
}

pub(super) async fn read_jsonrpc_message(stream: &mut WsClient) -> Result<JSONRPCMessage> {
    loop {
        let frame = timeout(DEFAULT_READ_TIMEOUT, stream.next())
            .await
            .context("timed out waiting for websocket frame")?
            .context("websocket stream ended unexpectedly")?
            .context("failed to read websocket frame")?;

        match frame {
            WebSocketMessage::Text(text) => return Ok(serde_json::from_str(text.as_ref())?),
            WebSocketMessage::Ping(payload) => {
                stream.send(WebSocketMessage::Pong(payload)).await?;
            }
            WebSocketMessage::Pong(_) => {}
            WebSocketMessage::Close(frame) => {
                bail!("websocket closed unexpectedly: {frame:?}")
            }
            WebSocketMessage::Binary(_) => bail!("unexpected binary websocket frame"),
            WebSocketMessage::Frame(_) => {}
        }
    }
}

pub(super) async fn assert_no_message(stream: &mut WsClient, wait_for: Duration) -> Result<()> {
    match timeout(wait_for, stream.next()).await {
        Ok(Some(Ok(frame))) => bail!("unexpected frame while waiting for silence: {frame:?}"),
        Ok(Some(Err(err))) => bail!("unexpected websocket read error: {err}"),
        Ok(None) => bail!("websocket closed unexpectedly while waiting for silence"),
        Err(_) => Ok(()),
    }
}

pub(super) fn create_config_toml(
    codex_home: &Path,
    server_uri: &str,
    approval_policy: &str,
) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "{approval_policy}"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}

fn connectable_bind_addr(bind_addr: SocketAddr) -> SocketAddr {
    match bind_addr {
        SocketAddr::V4(addr) if addr.ip().is_unspecified() => {
            SocketAddr::from(([127, 0, 0, 1], addr.port()))
        }
        SocketAddr::V6(addr) if addr.ip().is_unspecified() => {
            SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], addr.port()))
        }
        _ => bind_addr,
    }
}

fn signed_bearer_token(shared_secret: &[u8], claims: serde_json::Value) -> Result<String> {
    let header_segment = URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
    let claims_segment = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims)?);
    let payload = format!("{header_segment}.{claims_segment}");
    let mut mac = HmacSha256::new_from_slice(shared_secret).context("failed to create hmac")?;
    mac.update(payload.as_bytes());
    let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    Ok(format!("{payload}.{signature}"))
}
