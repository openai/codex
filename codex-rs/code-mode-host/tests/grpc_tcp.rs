use std::future::poll_fn;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use axum::http::Request;
use axum::http::StatusCode;
use axum::http::Version;
use codex_code_mode_protocol::grpc;
use codex_code_mode_protocol::grpc::code_mode_host_client::CodeModeHostClient;
use pretty_assertions::assert_eq;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::timeout;
use tonic::body::Body;
use tonic::codegen::Service;
use tonic::transport::Endpoint;

#[path = "support/host.rs"]
mod host;

use host::HostHarness;

const TEST_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 10);

#[tokio::test]
async fn tcp_listener_serves_http1_healthz() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let request = "GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    let response = http1_response(&host.endpoint, request.as_bytes()).await?;

    assert_eq!(response.lines().next(), Some("HTTP/1.1 200 OK"));
    Ok(())
}

#[tokio::test]
async fn tcp_listener_rejects_http1_grpc_requests() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let mut request = concat!(
        "POST /codex.code_mode.v1.CodeModeHost/OpenSession HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Content-Type: application/grpc\r\n",
        "Content-Length: 5\r\n",
        "Connection: close\r\n\r\n"
    )
    .as_bytes()
    .to_vec();
    request.extend_from_slice(&[0; 5]);
    let response = http1_response(&host.endpoint, &request).await?;

    assert_eq!(
        response.lines().next(),
        Some("HTTP/1.1 505 HTTP Version Not Supported")
    );
    Ok(())
}

#[tokio::test]
async fn tcp_listener_serves_http2_healthz() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let mut channel = Endpoint::from_shared(host.endpoint.clone())
        .context("gRPC code-mode host published an invalid endpoint")?
        .connect_timeout(TEST_TIMEOUT)
        .timeout(TEST_TIMEOUT)
        .connect()
        .await
        .context("failed to connect to gRPC code-mode host health endpoint")?;
    let request = Request::builder()
        .uri(format!("{}/healthz", host.endpoint))
        .body(Body::empty())
        .context("failed to build gRPC code-mode host health request")?;

    let response = timeout(TEST_TIMEOUT, async {
        poll_fn(|context| channel.poll_ready(context))
            .await
            .context("gRPC code-mode host health channel is unavailable")?;
        channel
            .call(request)
            .await
            .context("failed to request gRPC code-mode host health")
    })
    .await
    .context("timed out requesting gRPC code-mode host health")??;

    assert_eq!(response.version(), Version::HTTP_2);
    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}

async fn http1_response(endpoint: &str, request: &[u8]) -> Result<String> {
    let address = endpoint
        .strip_prefix("http://")
        .context("gRPC code-mode host URL should use http://")?;

    timeout(TEST_TIMEOUT, async {
        let mut stream = TcpStream::connect(address)
            .await
            .context("failed to connect to gRPC code-mode host")?;
        stream
            .write_all(request)
            .await
            .context("failed to send HTTP/1.1 request to gRPC code-mode host")?;

        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .await
            .context("failed to read HTTP/1.1 response from gRPC code-mode host")?;
        Ok(response)
    })
    .await
    .context("timed out waiting for HTTP/1.1 response from gRPC code-mode host")?
}

#[tokio::test]
async fn tcp_listener_opens_a_grpc_session() -> Result<()> {
    let mut host = Command::new(codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?)
        .args(["--listen", "grpc://127.0.0.1:0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(/*kill_on_drop*/ true)
        .spawn()
        .context("failed to start gRPC code-mode host")?;
    let stdout = host
        .stdout
        .take()
        .context("gRPC code-mode host stdout is unavailable")?;
    let mut stdout = BufReader::new(stdout);
    let mut endpoint = String::new();
    timeout(TEST_TIMEOUT, stdout.read_line(&mut endpoint))
        .await
        .context("gRPC code-mode host did not publish its endpoint")??;
    let endpoint = Endpoint::from_shared(endpoint.trim().to_string())
        .context("gRPC code-mode host published an invalid endpoint")?
        .connect_timeout(TEST_TIMEOUT)
        .timeout(TEST_TIMEOUT);
    let mut client = CodeModeHostClient::connect(endpoint)
        .await
        .context("failed to connect to gRPC code-mode host")?;
    let mut events = client
        .open_session(grpc::OpenSessionRequest {
            cell_execution_limits: None,
        })
        .await
        .context("failed to open gRPC code-mode session")?
        .into_inner();
    let event = timeout(TEST_TIMEOUT, events.message())
        .await
        .context("timed out waiting for gRPC code-mode session event")?
        .context("failed to read gRPC code-mode session event")?
        .context("gRPC code-mode session ended before opening")?;
    assert!(matches!(
        event.event,
        Some(grpc::session_event::Event::Opened(opened)) if !opened.session_id.is_empty()
    ));
    Ok(())
}
