//! Integration coverage for the remote Streamable HTTP RMCP path.
//!
//! These tests exercise the orchestrator-side RMCP adapter against a real
//! `exec-server` process so HTTP requests go through the remote runtime path
//! instead of direct local `reqwest` calls.

mod streamable_http_test_support;

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context as _;
use pretty_assertions::assert_eq;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::task::JoinHandle;

use streamable_http_test_support::call_echo_tool;
use streamable_http_test_support::create_remote_client;
use streamable_http_test_support::expected_echo_result;
use streamable_http_test_support::spawn_exec_server;
use streamable_http_test_support::spawn_streamable_http_server;

/// What this tests: the RMCP remote Streamable HTTP adapter can initialize
/// a server and call a tool while every MCP HTTP request goes through a real
/// exec-server process instead of a direct reqwest transport.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streamable_http_remote_client_round_trips_through_exec_server() -> anyhow::Result<()> {
    // Phase 1: start the MCP Streamable HTTP test server and a local
    // exec-server process that will own the HTTP network calls.
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let exec_server = spawn_exec_server().await?;

    // Phase 2: create and initialize the RMCP client using the executor-backed
    // Streamable HTTP transport.
    let client = create_remote_client(&base_url, exec_server.client.clone()).await?;

    // Phase 3: prove the initialized client can complete a tool call and
    // preserve the normal RMCP response shape.
    let result = call_echo_tool(&client, "remote").await?;
    assert_eq!(result, expected_echo_result("remote"));

    Ok(())
}

/// What this tests: when a real remote exec-server sees a no-status network
/// failure during the Streamable HTTP initialize request, it maps the reqwest
/// send failure into a JSON-RPC internal server error and the RMCP client still
/// treats that remote-shaped error as retryable.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streamable_http_remote_initialize_retries_no_response_failure() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let proxy = DropNextMcpPostProxy::spawn(&base_url).await?;
    proxy.arm_next_mcp_post_drop();
    let exec_server = spawn_exec_server().await?;

    let client = create_remote_client(proxy.base_url(), exec_server.client.clone()).await?;
    let result = call_echo_tool(&client, "remote-init-retry").await?;

    assert_eq!(proxy.dropped_mcp_posts(), 1);
    assert_eq!(result, expected_echo_result("remote-init-retry"));

    Ok(())
}

/// What this tests: once initialized through the real remote exec-server path,
/// a no-status Streamable HTTP failure during tools/list is retried instead of
/// surfacing the remote JSON-RPC internal server error to the caller.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streamable_http_remote_tools_list_retries_no_response_failure() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let proxy = DropNextMcpPostProxy::spawn(&base_url).await?;
    let exec_server = spawn_exec_server().await?;
    let client = create_remote_client(proxy.base_url(), exec_server.client.clone()).await?;

    proxy.arm_next_mcp_post_drop();
    let tools = client
        .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
        .await?;

    assert_eq!(proxy.dropped_mcp_posts(), 1);
    assert_eq!(tools.tools.len(), 1);
    assert_eq!(tools.tools[0].name, "echo");

    Ok(())
}

struct DropNextMcpPostProxy {
    base_url: String,
    drops_remaining: Arc<AtomicUsize>,
    dropped_mcp_posts: Arc<AtomicUsize>,
    task: JoinHandle<()>,
}

impl DropNextMcpPostProxy {
    async fn spawn(target_base_url: &str) -> anyhow::Result<Self> {
        let target_addr = parse_target_addr(target_base_url)?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let proxy_addr = listener.local_addr()?;
        let drops_remaining = Arc::new(AtomicUsize::new(0));
        let dropped_mcp_posts = Arc::new(AtomicUsize::new(0));
        let task_drops_remaining = Arc::clone(&drops_remaining);
        let task_dropped_mcp_posts = Arc::clone(&dropped_mcp_posts);

        let task = tokio::spawn(async move {
            while let Ok((client, _addr)) = listener.accept().await {
                let connection_drops_remaining = Arc::clone(&task_drops_remaining);
                let connection_dropped_mcp_posts = Arc::clone(&task_dropped_mcp_posts);
                tokio::spawn(async move {
                    let _ = proxy_connection(
                        client,
                        target_addr,
                        connection_drops_remaining,
                        connection_dropped_mcp_posts,
                    )
                    .await;
                });
            }
        });

        Ok(Self {
            base_url: format!("http://{proxy_addr}"),
            drops_remaining,
            dropped_mcp_posts,
            task,
        })
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn arm_next_mcp_post_drop(&self) {
        self.drops_remaining.fetch_add(1, Ordering::SeqCst);
    }

    fn dropped_mcp_posts(&self) -> usize {
        self.dropped_mcp_posts.load(Ordering::SeqCst)
    }
}

impl Drop for DropNextMcpPostProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn proxy_connection(
    mut client: TcpStream,
    target_addr: SocketAddr,
    drops_remaining: Arc<AtomicUsize>,
    dropped_mcp_posts: Arc<AtomicUsize>,
) -> anyhow::Result<()> {
    let request = read_http_message(&mut client).await?;
    if request.is_empty() {
        return Ok(());
    }

    if is_mcp_post(&request) && consume_drop(&drops_remaining) {
        dropped_mcp_posts.fetch_add(1, Ordering::SeqCst);
        return Ok(());
    }

    let request = with_connection_close(request)?;
    let mut upstream = TcpStream::connect(target_addr).await?;
    upstream.write_all(&request).await?;
    tokio::io::copy(&mut upstream, &mut client).await?;
    client.shutdown().await?;

    Ok(())
}

async fn read_http_message(stream: &mut TcpStream) -> anyhow::Result<Vec<u8>> {
    let mut message = Vec::new();
    let mut header_end = None;
    let mut chunk = [0_u8; 4096];

    while header_end.is_none() {
        let bytes_read = stream.read(&mut chunk).await?;
        if bytes_read == 0 {
            return Ok(message);
        }
        message.extend_from_slice(&chunk[..bytes_read]);
        header_end = find_header_end(&message);
    }

    let header_end = header_end.context("HTTP message headers were not terminated")?;
    let content_length = content_length(&message[..header_end])?;
    let message_len = header_end + content_length;

    while message.len() < message_len {
        let bytes_read = stream.read(&mut chunk).await?;
        if bytes_read == 0 {
            anyhow::bail!("HTTP message ended before body was complete");
        }
        message.extend_from_slice(&chunk[..bytes_read]);
    }

    Ok(message)
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| position + 4)
}

fn content_length(headers: &[u8]) -> anyhow::Result<usize> {
    let headers = std::str::from_utf8(headers).context("HTTP headers were not UTF-8")?;
    for line in headers.lines().skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            return value
                .trim()
                .parse::<usize>()
                .context("Content-Length header was not a usize");
        }
    }
    Ok(0)
}

fn is_mcp_post(request: &[u8]) -> bool {
    let Some(request_line) = std::str::from_utf8(request)
        .ok()
        .and_then(|request| request.lines().next())
    else {
        return false;
    };
    let mut parts = request_line.split_whitespace();
    parts.next() == Some("POST") && parts.next() == Some("/mcp")
}

fn consume_drop(drops_remaining: &AtomicUsize) -> bool {
    drops_remaining
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
            remaining.checked_sub(1)
        })
        .is_ok()
}

fn with_connection_close(request: Vec<u8>) -> anyhow::Result<Vec<u8>> {
    let header_end = find_header_end(&request).context("HTTP request headers were not complete")?;
    let headers = std::str::from_utf8(&request[..header_end]).context("request was not UTF-8")?;
    let mut next_request = Vec::with_capacity(request.len() + "Connection: close\r\n".len());

    for line in headers
        .strip_suffix("\r\n\r\n")
        .unwrap_or(headers)
        .split("\r\n")
    {
        if line
            .split_once(':')
            .is_some_and(|(name, _value)| name.eq_ignore_ascii_case("connection"))
        {
            continue;
        }
        next_request.extend_from_slice(line.as_bytes());
        next_request.extend_from_slice(b"\r\n");
    }
    next_request.extend_from_slice(b"Connection: close\r\n\r\n");
    next_request.extend_from_slice(&request[header_end..]);

    Ok(next_request)
}

fn parse_target_addr(base_url: &str) -> anyhow::Result<SocketAddr> {
    let url = reqwest::Url::parse(base_url)?;
    let host = url
        .host_str()
        .context("target URL did not include a host")?;
    let port = url
        .port_or_known_default()
        .context("target URL did not include a port")?;
    format!("{host}:{port}")
        .parse()
        .context("target URL did not resolve to a socket address")
}
