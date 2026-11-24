//! Dynamic-tool handlers finish their item lifecycle when cancellation is signalled.

use super::*;
use crate::state::ActiveTurn;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

async fn next_item(receiver: &async_channel::Receiver<Event>) -> DynamicToolCallItem {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let item = match receiver.recv().await.expect("event channel open").msg {
                EventMsg::ItemStarted(event) => event.item,
                EventMsg::ItemCompleted(event) => event.item,
                _ => continue,
            };
            if let TurnItem::DynamicToolCall(item) = item {
                return item;
            }
        }
    })
    .await
    .expect("dynamic tool item should be emitted")
}

#[tokio::test]
async fn response_cancel_race_completes_once() {
    let (session, turn, receiver) = crate::session::tests::make_session_and_context_with_rx().await;
    *session.active_turn.lock().await = Some(ActiveTurn::default());
    let cancellation = CancellationToken::new();
    let lifecycle = tokio::spawn({
        let session = Arc::clone(&session);
        let cancellation = cancellation.clone();
        async move {
            request_dynamic_tool(
                &session,
                &turn,
                "call-1".to_string(),
                ToolName::plain("gate"),
                json!({}),
                cancellation,
            )
            .await
        }
    });
    next_item(&receiver).await;
    let response =
        DynamicToolCallResult::ContentItems(codex_protocol::dynamic_tools::DynamicToolResponse {
            content_items: Vec::new(),
            success: true,
        });
    session
        .notify_dynamic_tool_response("call-1", response)
        .await;
    cancellation.cancel();
    let result = lifecycle.await.expect("lifecycle joined");
    assert!(result.as_ref().is_none_or(ToolOutput::success_for_logging));
    let item = next_item(&receiver).await;
    assert_eq!(
        (item.status, item.success, item.error),
        if result.is_some() {
            (DynamicToolCallStatus::Completed, Some(true), None)
        } else {
            (
                DynamicToolCallStatus::Failed,
                Some(false),
                Some("dynamic tool call was cancelled before receiving a response".to_string()),
            )
        }
    );
    while let Ok(event) = receiver.try_recv() {
        assert!(!matches!(event.msg, EventMsg::ItemCompleted(_)));
    }
}

#[test]
fn native_reply_has_the_same_code_mode_and_hook_payload() {
    let fields = json!({"threads": [{"id":"abc"}]});
    let (output, items) =
        DynamicToolOutput::from_response(DynamicToolCallResult::StructuredContent {
            structured_content: fields.clone(),
            success: true,
        });
    let payload = ToolPayload::Function {
        arguments: "{}".to_string(),
    };
    let result = codex_protocol::mcp::CallToolResult {
        content: vec![json!({"type": "text", "text": fields.to_string()})],
        structured_content: Some(fields.clone()),
        is_error: Some(false),
        meta: None,
    }
    .code_mode_result(&payload);
    assert_eq!(output.code_mode_result(&payload), result);
    assert_eq!(
        output.post_tool_use_response("call-1", &payload),
        Some(result.clone().into_json())
    );
    assert_eq!(Box::new(output).into_code_mode_result(&payload), result);
    assert_eq!(
        items,
        vec![DynamicToolCallOutputContentItem::InputText {
            text: fields.to_string()
        }]
    );
}

#[test]
fn native_reply_moves_the_original_payload_into_code_mode() {
    let text = "large tool reply".repeat(1024);
    let original = text.as_ptr();
    let (output, _) = DynamicToolOutput::from_response(DynamicToolCallResult::StructuredContent {
        structured_content: Value::String(text),
        success: true,
    });
    let output: Box<dyn ToolOutput> = Box::new(output);
    let CodeModeToolResult::McpCallToolResult(result) =
        output.into_code_mode_result(&ToolPayload::Function {
            arguments: "{}".to_owned(),
        })
    else {
        panic!("native reply lost its MCP result tag");
    };
    assert_eq!(
        result["structuredContent"].as_str().unwrap().as_ptr(),
        original
    );
}
