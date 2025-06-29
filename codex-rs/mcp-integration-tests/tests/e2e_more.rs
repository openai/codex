use codex_mcp_client::McpClient;
use codex_mcp_server::run_with_streams;
use mcp_types::{
    InitializeRequestParams,
    ClientCapabilities,
    Implementation,
    CallToolResultContent,
    JSONRPCMessage,
    JSONRPCRequest,
    JSONRPCResponse,
    JSONRPCBatchRequestItem,
    RequestId,
    ListToolsResult,
    JSONRPC_VERSION,
};
use std::time::Duration;
use serde_json;
use tokio::io::{duplex, split, AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Helper to set up an in-memory client-server pair.
async fn setup() -> McpClient {
    let (client_stream, server_stream) = duplex(1 << 16);
    let (client_read, client_write) = split(client_stream);
    let (server_read, server_write) = split(server_stream);
    // Spawn server
    tokio::spawn(async move {
        run_with_streams(BufReader::new(server_read), server_write, None)
            .await
            .unwrap();
    });
    // Return client
    (McpClient::with_streams(BufReader::new(client_read), client_write))
}

#[tokio::test]
async fn call_missing_args_yields_error() {
    let client = setup().await;
    // Call codex tool without args
    let res = client.call_tool("codex".to_string(), None, Some(Duration::from_secs(5))).await.unwrap();
    assert_eq!(res.is_error, Some(true));
    if let CallToolResultContent::TextContent(tc) = &res.content[0] {
        assert!(tc.text.contains("Missing arguments"));
    } else {
        panic!("Expected text content for missing args");
    }
}
/// Two clients listing tools concurrently should both succeed independently
#[tokio::test]
async fn concurrent_list_tools() {
    let client1 = setup().await;
    let client2 = setup().await;
    let (res1, res2) = tokio::join!(
        client1.list_tools(None, Some(Duration::from_secs(5))),
        client2.list_tools(None, Some(Duration::from_secs(5)))
    );
    let list1 = res1.unwrap();
    let list2 = res2.unwrap();
    assert!(list1.tools.iter().any(|t| t.name == "codex"));
    assert!(list2.tools.iter().any(|t| t.name == "codex"));
}

/// Send a batch request (ping + list) and expect two individual responses.
#[tokio::test]
async fn batch_ping_and_list_tools() {
    use mcp_types::{JSONRPCMessage, JSONRPCBatchRequest, JSONRPCBatchRequestItem, JSONRPCRequest, JSONRPCResponse};
    // Setup in-memory server/client
    let (client_stream, server_stream) = duplex(1 << 16);
    let (client_read_half, mut client_write) = split(client_stream);
    let mut reader = BufReader::new(client_read_half);
    let (server_read, server_write) = split(server_stream);
    // Spawn server
    tokio::spawn(async move {
        run_with_streams(BufReader::new(server_read), server_write, None)
            .await
            .unwrap();
    });
    // Build batch: ping(id=100), tools/list(id=101)
    let req_ping = JSONRPCRequest { id: RequestId::Integer(100), jsonrpc: JSONRPC_VERSION.to_string(), method: "ping".to_string(), params: None };
    let req_list = JSONRPCRequest { id: RequestId::Integer(101), jsonrpc: JSONRPC_VERSION.to_string(), method: "tools/list".to_string(), params: None };
    let batch = JSONRPCMessage::BatchRequest(vec![
        JSONRPCBatchRequestItem::JSONRPCRequest(req_ping.clone()),
        JSONRPCBatchRequestItem::JSONRPCRequest(req_list.clone()),
    ]);
    // Send batch
    let batch_json = serde_json::to_string(&batch).unwrap();
    client_write.write_all(batch_json.as_bytes()).await.unwrap();
    client_write.write_all(b"\n").await.unwrap();
    client_write.flush().await.unwrap();
    // Read two responses
    let mut got_ids = vec![];
    for _ in 0..2 {
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let msg: JSONRPCMessage = serde_json::from_str(line.trim()).unwrap();
        if let JSONRPCMessage::Response(JSONRPCResponse { id, result, .. }) = msg {
            if id == RequestId::Integer(100) {
                assert_eq!(result, serde_json::Value::Object(Default::default()));
            } else if id == RequestId::Integer(101) {
                let list: ListToolsResult = serde_json::from_value(result).unwrap();
                assert!(list.tools.iter().any(|t| t.name == "codex"));
            } else {
                panic!("unexpected response id: {:?}", id);
            }
            got_ids.push(id);
        } else {
            panic!("expected response, got {:?}", msg);
        }
    }
    assert_eq!(got_ids.len(), 2);
}

/// Server should ignore malformed JSON and recover for the next valid request.
#[tokio::test]
async fn malformed_json_recovery() {
    use mcp_types::JSONRPCRequest;
    let (client_stream, server_stream) = duplex(1 << 16);
    let (client_read_half, mut client_write) = split(client_stream);
    let mut reader = BufReader::new(client_read_half);
    let (server_read, server_write) = split(server_stream);
    tokio::spawn(async move {
        run_with_streams(BufReader::new(server_read), server_write, None)
            .await
            .unwrap();
    });
    // Send invalid JSON
    client_write.write_all(b"}{ invalid json \n").await.unwrap();
    client_write.flush().await.unwrap();
    // Then send a valid ping
    let req = JSONRPCRequest { id: RequestId::Integer(200), jsonrpc: JSONRPC_VERSION.to_string(), method: "ping".to_string(), params: None };
    let json_req = serde_json::to_string(&JSONRPCMessage::Request(req.clone())).unwrap();
    client_write.write_all(json_req.as_bytes()).await.unwrap();
    client_write.write_all(b"\n").await.unwrap();
    client_write.flush().await.unwrap();
    // Read one response
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    let msg: JSONRPCMessage = serde_json::from_str(line.trim()).unwrap();
    if let JSONRPCMessage::Response(resp) = msg {
        assert_eq!(resp.id, req.id);
    } else {
        panic!("expected ping response, got {:?}", msg);
    }
}

/// Stub server returns multiple text fragments in a single call_tool.
#[tokio::test]
async fn stub_call_multiple_content() {
    use mcp_types::{JSONRPCMessage, JSONRPCResponse, CallToolResult, CallToolResultContent, TextContent};
    // Set up in-memory duplex
    let (client_stream, server_stream) = duplex(1 << 16);
    let (client_read, mut client_write) = split(client_stream);
    let (server_read, mut server_write) = split(server_stream);
    let mut reader = BufReader::new(server_read);

    // Spawn stub server
    tokio::spawn(async move {
        // Read request
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let msg: JSONRPCMessage = serde_json::from_str(line.trim()).unwrap();
        if let JSONRPCMessage::Request(req) = msg {
            let id = req.id;
            // Prepare multiple content items
            let contents = vec![
                CallToolResultContent::TextContent(TextContent { r#type: "text".into(), text: "first".into(), annotations: None }),
                CallToolResultContent::TextContent(TextContent { r#type: "text".into(), text: "second".into(), annotations: None }),
            ];
            let result = CallToolResult { content: contents.clone(), is_error: None };
            let resp = JSONRPCMessage::Response(JSONRPCResponse {
                jsonrpc: JSONRPC_VERSION.to_string(),
                id,
                result: serde_json::to_value(result).unwrap(),
            });
            let out = serde_json::to_string(&resp).unwrap();
            server_write.write_all(out.as_bytes()).await.unwrap();
            server_write.write_all(b"\n").await.unwrap();
            server_write.flush().await.unwrap();
        }
    });
    // Create client
    let client = McpClient::with_streams(BufReader::new(client_read), client_write);
    // Call tool and expect two fragments
    let res = client.call_tool("foobar".into(), None, Some(Duration::from_secs(5))).await.unwrap();
    assert_eq!(res.content.len(), 2);
    if let CallToolResultContent::TextContent(tc) = &res.content[0] {
        assert_eq!(tc.text, "first");
    } else { panic!("unexpected content[0]"); }
    if let CallToolResultContent::TextContent(tc) = &res.content[1] {
        assert_eq!(tc.text, "second");
    } else { panic!("unexpected content[1]"); }
}

/// Sending tools/call with an unknown tool name yields an error message.
#[tokio::test]
async fn unknown_tool_call() {
    let client = setup().await;
    // Call a non-existent tool
    let res = client.call_tool("foobar".to_string(), None, Some(Duration::from_secs(5))).await.unwrap();
    assert_eq!(res.is_error, Some(true));
    if let CallToolResultContent::TextContent(tc) = &res.content[0] {
        assert!(tc.text.contains("Unknown tool 'foobar'"));
    } else {
        panic!("Expected text content for unknown tool error");
    }
}

/// Two concurrent call_tool requests should not interfere.
#[tokio::test]
async fn concurrent_call_tool_errors() {
    let c1 = setup().await;
    let c2 = setup().await;
    let (r1, r2) = tokio::join!(
        c1.call_tool("foobar".to_string(), None, Some(Duration::from_secs(5))),
        c2.call_tool("baz".to_string(), None, Some(Duration::from_secs(5)))
    );
    let res1 = r1.unwrap();
    let res2 = r2.unwrap();
    assert!(matches!(res1.content[0], CallToolResultContent::TextContent(_)));
    assert!(matches!(res2.content[0], CallToolResultContent::TextContent(_)));
}