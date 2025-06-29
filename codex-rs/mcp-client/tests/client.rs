use tokio::io::{duplex, split, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::task;
use std::time::Duration;
use codex_mcp_client::McpClient;
use mcp_types::{JSONRPCMessage, JSONRPCResponse, ListToolsResult, Tool, ToolInputSchema, CallToolRequestParams, CallToolResult, CallToolResultContent, TextContent, JSONRPC_VERSION};
use serde_json::json;

#[tokio::test]
async fn test_list_and_call() {
    // Create in-memory duplex stream
    let (client_io, server_io) = duplex(1 << 16);
    let (client_read, client_write) = split(client_io);
    let (server_read, mut server_write) = split(server_io);

    // Spawn stub MCP server
    let server_task = task::spawn(async move {
        let mut reader = BufReader::new(server_read);
        let mut buf = String::new();

        // Handle tools/list request
        buf.clear();
        reader.read_line(&mut buf).await.unwrap();
        let msg: JSONRPCMessage = serde_json::from_str(buf.trim()).unwrap();
        let req = match msg {
            JSONRPCMessage::Request(r) => r,
            _ => panic!("expected Request for tools/list"),
        };
        assert_eq!(req.method, "tools/list");
        let id = req.id.clone();
        // Respond with single 'echo' tool
        let tool = Tool {
            name: "echo".to_string(),
            input_schema: ToolInputSchema { properties: None, required: None, r#type: "object".to_string() },
            description: None,
            annotations: None,
        };
        let result = ListToolsResult { tools: vec![tool], next_cursor: None };
        let response = JSONRPCResponse { id, jsonrpc: JSONRPC_VERSION.to_string(), result: serde_json::to_value(result).unwrap() };
        let out = JSONRPCMessage::Response(response);
        let out_json = serde_json::to_string(&out).unwrap();
        server_write.write_all(out_json.as_bytes()).await.unwrap();
        server_write.write_all(b"\n").await.unwrap();

        // Handle tools/call request
        buf.clear();
        reader.read_line(&mut buf).await.unwrap();
        let msg2: JSONRPCMessage = serde_json::from_str(buf.trim()).unwrap();
        let req2 = match msg2 {
            JSONRPCMessage::Request(r) => r,
            _ => panic!("expected Request for tools/call"),
        };
        assert_eq!(req2.method, "tools/call");
        let id2 = req2.id.clone();
        let params: CallToolRequestParams = serde_json::from_value(req2.params.unwrap()).unwrap();
        assert_eq!(params.name, "echo");
        let args = params.arguments.unwrap();
        let arg = args["msg"].as_str().unwrap();
        assert_eq!(arg, "hi");
        // Echo back the message
        let content = CallToolResultContent::TextContent(TextContent { r#type: "text".to_string(), text: arg.to_string(), annotations: None });
        let result2 = CallToolResult { content: vec![content], is_error: None };
        let response2 = JSONRPCResponse { id: id2, jsonrpc: JSONRPC_VERSION.to_string(), result: serde_json::to_value(result2).unwrap() };
        let out2 = JSONRPCMessage::Response(response2);
        let out2_json = serde_json::to_string(&out2).unwrap();
        server_write.write_all(out2_json.as_bytes()).await.unwrap();
        server_write.write_all(b"\n").await.unwrap();
    });

    // Create client over in-memory streams
    let client = McpClient::with_streams(BufReader::new(client_read), client_write);
    // Test tools/list
    let list = client.list_tools(None, Some(Duration::from_secs(1))).await.unwrap();
    assert_eq!(list.tools.len(), 1);
    assert_eq!(list.tools[0].name, "echo");
    // Test tools/call
    let call_res = client.call_tool("echo".to_string(), Some(json!({"msg":"hi"})), Some(Duration::from_secs(1))).await.unwrap();
    match &call_res.content[0] {
        CallToolResultContent::TextContent(tc) => assert_eq!(tc.text, "hi"),
        other => panic!("unexpected content: {:?}", other),
    }
    server_task.await.unwrap();
}

#[tokio::test]
async fn test_notifications() {
    use mcp_types::JSONRPCNotification;
    use serde_json::json;
    // Prepare in-memory streams
    let (client_io, server_io) = duplex(1 << 16);
    let (client_read, client_write) = split(client_io);
    let (server_read, mut server_write) = split(server_io);
    // Spawn stub server to emit two notifications
    task::spawn(async move {
        // Notification 1
        let note1 = JSONRPCNotification {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: "foo".to_string(),
            params: Some(json!({ "a": 1 })),
        };
        let msg1 = JSONRPCMessage::Notification(note1.clone());
        let out1 = serde_json::to_string(&msg1).unwrap();
        server_write.write_all(out1.as_bytes()).await.unwrap();
        server_write.write_all(b"\n").await.unwrap();
        // Notification 2
        let note2 = JSONRPCNotification {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: "bar".to_string(),
            params: Some(json!({ "b": 2 })),
        };
        let msg2 = JSONRPCMessage::Notification(note2.clone());
        let out2 = serde_json::to_string(&msg2).unwrap();
        server_write.write_all(out2.as_bytes()).await.unwrap();
        server_write.write_all(b"\n").await.unwrap();
        // Finally, send a dummy response so reader can finish
        let resp = JSONRPCMessage::Response(JSONRPCResponse {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: req_id(),
            result: json!("ok"),
        });
        let out3 = serde_json::to_string(&resp).unwrap();
        server_write.write_all(out3.as_bytes()).await.unwrap();
        server_write.write_all(b"\n").await.unwrap();
    });
    // Client
    let mut client = McpClient::with_streams(BufReader::new(client_read), client_write);
    // Receive notifications
    let n1 = client.next_notification().await.unwrap();
    assert_eq!(n1.method, "foo");
    let n2 = client.next_notification().await.unwrap();
    assert_eq!(n2.method, "bar");
}

// Helper to produce a dummy RequestId for the dummy response
fn req_id() -> mcp_types::RequestId {
    mcp_types::RequestId::Integer(0)
}