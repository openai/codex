//! Integration coverage for the remote Streamable HTTP RMCP path.
//!
//! These tests exercise the orchestrator-side RMCP adapter against a real
//! `exec-server` process so HTTP requests go through the remote runtime path
//! instead of direct local `reqwest` calls.

mod streamable_http_test_support;

use pretty_assertions::assert_eq;

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
