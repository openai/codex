use codex_mcp_client::McpClient;
use codex_mcp_server::run_with_streams;
use mcp_types::{InitializeRequestParams, ClientCapabilities, Implementation};
use std::time::Duration;
use tokio::io::{duplex, split, AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn e2e_initialize_and_list_tools() {
    // Set up in-memory bidirectional stream
    let (client_stream, server_stream) = duplex(1 << 16);
    let (client_read, client_write) = split(client_stream);
    let (server_read, server_write) = split(server_stream);

    // Spawn the server over the server half
    let server_task = tokio::spawn(async move {
        run_with_streams(BufReader::new(server_read), server_write, None)
            .await
            .unwrap();
    });

    // Create the client over the client half
    let client = McpClient::with_streams(BufReader::new(client_read), client_write);

    // Send initialize
    let params = InitializeRequestParams {
        capabilities: ClientCapabilities { experimental: None, roots: None, sampling: None },
        client_info: Implementation { name: "integration-test".to_string(), version: "0.1.0".to_string() },
        protocol_version: mcp_types::MCP_SCHEMA_VERSION.to_string(),
    };
    let init_res = client.initialize(params, None, Some(Duration::from_secs(5))).await.unwrap();
    assert_eq!(init_res.protocol_version, mcp_types::MCP_SCHEMA_VERSION);

    // List tools and check for 'codex'
    let list = client.list_tools(None, Some(Duration::from_secs(5))).await.unwrap();
    assert!(list.tools.iter().any(|t| t.name == "codex"));

    // Drop client to close stream and signal server ingress end
    drop(client);
    // Leave server task running; it will exit when its reader sees EOF.
}