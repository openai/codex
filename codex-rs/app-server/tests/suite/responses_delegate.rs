use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::AddConversationListenerParams;
use codex_app_server_protocol::AddConversationSubscriptionResponse;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::NewConversationResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ResponsesApiCallResponse;
use codex_app_server_protocol::ResponsesApiEventParams;
use codex_app_server_protocol::SendUserMessageParams;
use codex_app_server_protocol::SendUserMessageResponse;
use codex_app_server_protocol::ServerRequest;
use codex_protocol::ConversationId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn test_delegate_responses_over_jsonrpc() -> Result<()> {
    // Create temp Codex home and enable the JSON-RPC delegation feature.
    let codex_home = TempDir::new()?;
    write_feature_flag(codex_home.path())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Start a conversation using defaults (OpenAI provider: Responses API).
    let new_conv_id = mcp
        .send_new_conversation_request(NewConversationParams { ..Default::default() })
        .await?;
    let new_conv_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(new_conv_id)),
    )
    .await??;
    let NewConversationResponse { conversation_id, .. } = to_response::<_>(new_conv_resp)?;

    // Subscribe to conversation events (raw) so we can assert stream behaviour.
    let add_listener_id = mcp
        .send_add_conversation_listener_request(AddConversationListenerParams {
            conversation_id,
            experimental_raw_events: true,
        })
        .await?;
    let add_listener_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(add_listener_id)),
    )
    .await??;
    let _add_listener_ok: AddConversationSubscriptionResponse =
        to_response::<_>(add_listener_resp)?;

    // Kick off a user message – expect two delegated calls (session start and message).
    let send_id = mcp
        .send_send_user_message_request(SendUserMessageParams {
            conversation_id,
            items: vec![codex_app_server_protocol::InputItem::Text {
                text: "Hello from test".to_string(),
            }],
        })
        .await?;

    for _ in 0..2 {
        let request = mcp.read_stream_until_request_message().await?;
        let ServerRequest::ResponsesApiCall { request_id, params } = request else {
            panic!("expected ResponsesApiCall request");
        };

        // Stream Responses API events back to the server.
        let created = serde_json::json!({
            "type": "response.created",
            "response": {"id": "resp_test"}
        });
        let msg = serde_json::json!({
            "type": "response.output_item.done",
            "item": {
                "type": "message",
                "role": "assistant",
                "content": [{"type":"output_text","text":"Done"}]
            }
        });
        let completed = serde_json::json!({
            "type": "response.completed",
            "response": {"id": "resp_test"}
        });

        mcp
            .send_notification(ClientNotification::ResponsesApiEvent(
                ResponsesApiEventParams { call_id: params.call_id.clone(), event: created },
            ))
            .await?;
        mcp
            .send_notification(ClientNotification::ResponsesApiEvent(
                ResponsesApiEventParams { call_id: params.call_id.clone(), event: msg },
            ))
            .await?;
        mcp
            .send_notification(ClientNotification::ResponsesApiEvent(
                ResponsesApiEventParams { call_id: params.call_id.clone(), event: completed },
            ))
            .await?;

        // Finalize the delegated request.
        let result = serde_json::to_value(ResponsesApiCallResponse {
            status: 200,
            request_id: None,
            error: None,
        })?;
        mcp.send_response(request_id, result).await?;
    }

    // Verify sendUserMessage returns OK.
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(send_id)),
    )
    .await??;
    let _ok: SendUserMessageResponse = to_response::<_>(resp)?;

    // Expect at least one raw output item matching assistant Done.
    let raw = read_raw_item(&mut mcp, conversation_id).await;
    assert!(matches!(
        raw,
        ResponseItem::Message { role, content, .. }
        if role == "assistant" && content.iter().any(|c| matches!(c, ContentItem::OutputText { text } if text == "Done"))
    ));

    Ok(())
}

fn write_feature_flag(codex_home: &std::path::Path) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::create_dir_all(codex_home)?;
    std::fs::write(
        config_toml,
        r#"[features]
responses_http_over_jsonrpc = true
"#,
    )
}

async fn read_raw_item(mcp: &mut McpProcess, conversation_id: ConversationId) -> ResponseItem {
    let notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("codex/event/raw_response_item"),
    )
    .await
    .expect("raw item notify")
    .expect("raw item notify inner");

    let params = notification.params.expect("params");
    let serde_json::Value::Object(map) = params else { panic!("object") };
    assert_eq!(
        map.get("conversationId"),
        Some(&serde_json::Value::String(conversation_id.to_string()))
    );
    let item_val = map.get("item").cloned().expect("item");
    serde_json::from_value::<ResponseItem>(item_val).expect("response item")
}

