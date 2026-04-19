#![cfg(unix)]

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpRequestBodyDeltaNotification;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::InitializeParams;
use common::exec_server::ExecServerHarness;
use common::exec_server::exec_server;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::timeout;

/// HTTP request captured by the ad-hoc TCP server in these integration tests.
#[derive(Debug)]
struct CapturedHttpRequest {
    stream: TcpStream,
    request_line: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

/// What this tests: a real exec-server websocket `http/request` performs one
/// HTTP request through the runner and returns the complete response body in
/// the JSON-RPC response.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_http_request_buffers_response_body() -> anyhow::Result<()> {
    // Phase 1: start exec-server and complete the JSON-RPC handshake.
    let mut server = exec_server().await?;
    initialize_exec_server(&mut server).await?;

    // Phase 2: start a local HTTP peer and ask exec-server to POST to it.
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}/mcp?case=buffered", listener.local_addr()?);
    let http_request_id = server
        .send_request(
            "http/request",
            serde_json::to_value(HttpRequestParams {
                method: "POST".to_string(),
                url,
                headers: vec![HttpHeader {
                    name: "x-codex-test".to_string(),
                    value: "buffered".to_string(),
                }],
                body: Some(b"request-body".to_vec().into()),
                timeout_ms: Some(5_000),
                request_id: None,
                stream_response: false,
            })?,
        )
        .await?;

    // Phase 3: assert the HTTP peer observes the expected method, path,
    // headers, and body before returning a fixed-length response.
    let captured = accept_http_request(&listener).await?;
    assert_eq!(
        (
            captured.request_line.as_str(),
            captured.headers.get("x-codex-test").map(String::as_str),
            captured.body.as_slice(),
        ),
        (
            "POST /mcp?case=buffered HTTP/1.1",
            Some("buffered"),
            b"request-body".as_slice(),
        )
    );
    respond_with_status_and_headers(
        captured.stream,
        "201 Created",
        &[("x-mcp-test", "buffered")],
        b"response-body",
    )
    .await?;

    // Phase 4: assert exec-server returns status, response headers, and the
    // full response body in the JSON-RPC result.
    let response: HttpRequestResponse = wait_for_response(&mut server, http_request_id).await?;
    assert_eq!(
        (
            response.status,
            response_header(&response.headers, "x-mcp-test"),
            response.body.into_inner(),
        ),
        (201, Some("buffered".to_string()), b"response-body".to_vec(),)
    );

    server.shutdown().await?;
    Ok(())
}

/// What this tests: a real exec-server websocket `http/request` can return
/// response headers immediately and stream the response body as ordered
/// `http/request/bodyDelta` notifications.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_http_request_streams_response_body_notifications() -> anyhow::Result<()> {
    // Phase 1: start exec-server and complete the JSON-RPC handshake.
    let mut server = exec_server().await?;
    initialize_exec_server(&mut server).await?;

    // Phase 2: start a local HTTP peer and ask exec-server for a streamed GET.
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}/mcp?case=streaming", listener.local_addr()?);
    let http_request_id = server
        .send_request(
            "http/request",
            serde_json::to_value(HttpRequestParams {
                method: "GET".to_string(),
                url,
                headers: vec![HttpHeader {
                    name: "accept".to_string(),
                    value: "text/event-stream".to_string(),
                }],
                body: None,
                timeout_ms: Some(5_000),
                request_id: Some("stream-1".to_string()),
                stream_response: true,
            })?,
        )
        .await?;

    // Phase 3: assert the HTTP peer observes the expected request and then
    // respond with chunked transfer encoding to exercise streaming.
    let captured = accept_http_request(&listener).await?;
    assert_eq!(
        (
            captured.request_line.as_str(),
            captured.headers.get("accept").map(String::as_str),
            captured.body,
        ),
        (
            "GET /mcp?case=streaming HTTP/1.1",
            Some("text/event-stream"),
            Vec::new(),
        )
    );
    respond_with_chunked_body(
        captured.stream,
        &[("x-mcp-test", "streaming")],
        &[b"hello ".as_slice(), b"world".as_slice()],
    )
    .await?;

    // Phase 4: assert the JSON-RPC response contains status and headers but no
    // buffered body when streaming is requested.
    let response: HttpRequestResponse = wait_for_response(&mut server, http_request_id).await?;
    assert_eq!(
        (
            response.status,
            response_header(&response.headers, "x-mcp-test"),
            response.body.into_inner(),
        ),
        (200, Some("streaming".to_string()), Vec::new())
    );

    // Phase 5: assert the body notifications are contiguous, ordered, and end
    // with a clean terminal frame.
    let deltas = collect_response_body_deltas(&mut server, "stream-1").await?;
    let seqs = deltas.iter().map(|delta| delta.seq).collect::<Vec<_>>();
    let body = deltas
        .iter()
        .flat_map(|delta| delta.delta.clone().into_inner())
        .collect::<Vec<_>>();
    let terminal = deltas.last().map(|delta| (delta.done, delta.error.clone()));
    let expected_seqs = (1..=deltas.len() as u64).collect::<Vec<_>>();
    assert_eq!(
        (seqs, body, terminal),
        (expected_seqs, b"hello world".to_vec(), Some((true, None)))
    );

    server.shutdown().await?;
    Ok(())
}

/// Performs the JSON-RPC initialize handshake required before executor methods.
async fn initialize_exec_server(server: &mut ExecServerHarness) -> anyhow::Result<()> {
    let initialize_id = server
        .send_request(
            "initialize",
            serde_json::to_value(InitializeParams {
                client_name: "exec-server-http-test".to_string(),
                resume_session_id: None,
            })?,
        )
        .await?;
    let _: Value = wait_for_response(server, initialize_id).await?;
    server
        .send_notification("initialized", serde_json::json!({}))
        .await?;
    Ok(())
}

/// Waits for a typed JSON-RPC response with the requested id.
async fn wait_for_response<T>(
    server: &mut ExecServerHarness,
    request_id: RequestId,
) -> anyhow::Result<T>
where
    T: DeserializeOwned,
{
    let response = server
        .wait_for_event(|event| {
            matches!(
                event,
                JSONRPCMessage::Response(JSONRPCResponse { id, .. }) if id == &request_id
            )
        })
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { result, .. }) = response else {
        anyhow::bail!("expected JSON-RPC response for {request_id:?}");
    };
    Ok(serde_json::from_value(result)?)
}

/// Accepts one HTTP/1.1 request and captures its wire-visible fields.
async fn accept_http_request(listener: &TcpListener) -> anyhow::Result<CapturedHttpRequest> {
    let (stream, _) = timeout(Duration::from_secs(5), listener.accept()).await??;
    let mut reader = BufReader::new(stream);

    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;
    let request_line = request_line.trim_end_matches("\r\n").to_string();

    let mut headers = BTreeMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        if line == "\r\n" {
            break;
        }
        let line = line.trim_end_matches("\r\n");
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("HTTP header should contain colon: {line}"))?;
        headers.insert(name.to_ascii_lowercase(), value.trim().to_string());
    }

    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).await?;

    Ok(CapturedHttpRequest {
        stream: reader.into_inner(),
        request_line,
        headers,
        body,
    })
}

/// Writes a fixed-length HTTP response to the captured request stream.
async fn respond_with_status_and_headers(
    mut stream: TcpStream,
    status: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> anyhow::Result<()> {
    let extra_headers = headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}\r\n"))
        .collect::<String>();
    let response = format!(
        "HTTP/1.1 {status}\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n{extra_headers}\r\n",
        body.len(),
    );
    stream.write_all(response.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
}

/// Writes a chunked HTTP response so reqwest must drive the streaming path.
async fn respond_with_chunked_body(
    mut stream: TcpStream,
    headers: &[(&str, &str)],
    chunks: &[&[u8]],
) -> anyhow::Result<()> {
    let extra_headers = headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}\r\n"))
        .collect::<String>();
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ntransfer-encoding: chunked\r\nconnection: close\r\n{extra_headers}\r\n",
    );
    stream.write_all(response.as_bytes()).await?;
    for chunk in chunks {
        stream
            .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
            .await?;
        stream.write_all(chunk).await?;
        stream.write_all(b"\r\n").await?;
        stream.flush().await?;
    }
    stream.write_all(b"0\r\n\r\n").await?;
    stream.flush().await?;
    Ok(())
}

/// Collects streamed response-body notifications until the terminal frame.
async fn collect_response_body_deltas(
    server: &mut ExecServerHarness,
    request_id: &str,
) -> anyhow::Result<Vec<HttpRequestBodyDeltaNotification>> {
    let mut deltas = Vec::new();
    loop {
        let event = server.next_event().await?;
        let JSONRPCMessage::Notification(JSONRPCNotification { method, params }) = event else {
            anyhow::bail!("expected http/request body delta notification, got {event:?}");
        };
        assert_eq!(method, "http/request/bodyDelta");
        let delta: HttpRequestBodyDeltaNotification =
            serde_json::from_value(params.unwrap_or(Value::Null))?;
        assert_eq!(delta.request_id, request_id);

        let done = delta.done;
        deltas.push(delta);
        if done {
            return Ok(deltas);
        }
    }
}

/// Returns a response header value without depending on header-name casing.
fn response_header(headers: &[HttpHeader], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.clone())
}
