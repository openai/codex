use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::tempdir;

#[test]
fn failed_turn_does_not_overwrite_output_last_message_file() {
    let tempdir = tempdir().expect("create tempdir");
    let output_path = tempdir.path().join("last-message.txt");
    std::fs::write(&output_path, "keep existing contents").expect("seed output file");

    let mut processor = EventProcessorWithJsonOutput::new(Some(output_path.clone()));

    let collected = processor.collect_thread_events(ServerNotification::ItemCompleted(
        codex_app_server_protocol::ItemCompletedNotification {
            item: ThreadItem::AgentMessage {
                id: "msg-1".to_string(),
                text: "partial answer".to_string(),
                phase: None,
                memory_citation: None,
            },
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            completed_at_ms: 0,
        },
    ));

    assert_eq!(collected.status, CodexStatus::Running);
    assert_eq!(processor.final_message(), Some("partial answer"));

    let status = processor.process_server_notification(ServerNotification::TurnCompleted(
        codex_app_server_protocol::TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: codex_app_server_protocol::Turn {
                id: "turn-1".to_string(),
                items_view: codex_app_server_protocol::TurnItemsView::Full,
                items: Vec::new(),
                status: TurnStatus::Failed,
                error: Some(codex_app_server_protocol::TurnError {
                    message: "turn failed".to_string(),
                    additional_details: None,
                    codex_error_info: None,
                }),
                started_at: None,
                completed_at: Some(0),
                duration_ms: None,
            },
        },
    ));

    assert_eq!(status, CodexStatus::InitiateShutdown);
    assert_eq!(processor.final_message(), None);

    EventProcessor::print_final_output(&mut processor);

    assert_eq!(
        std::fs::read_to_string(&output_path).expect("read output file"),
        "keep existing contents"
    );
}

#[test]
fn mcp_tool_call_result_preserves_meta_in_jsonl_event() {
    let mut processor = EventProcessorWithJsonOutput::new(/*last_message_path*/ None);

    let collected = processor.collect_thread_events(ServerNotification::ItemCompleted(
        codex_app_server_protocol::ItemCompletedNotification {
            item: ThreadItem::McpToolCall {
                id: "mcp-1".to_string(),
                server: "search service".to_string(),
                tool: "web_run".to_string(),
                status: McpToolCallStatus::Completed,
                arguments: json!({"search_query": [{"q": "OpenAI Codex CLI documentation"}]}),
                mcp_app_resource_uri: None,
                result: Some(Box::new(codex_app_server_protocol::McpToolCallResult {
                    content: vec![json!({"type": "text", "text": "search result"})],
                    structured_content: None,
                    meta: Some(json!({"raw_messages": [{"ref_id": "turn0search0"}]})),
                })),
                error: None,
                duration_ms: Some(42),
            },
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            completed_at_ms: 0,
        },
    ));

    assert_eq!(collected.status, CodexStatus::Running);
    assert_eq!(collected.events.len(), 1);

    let ThreadEvent::ItemCompleted(ItemCompletedEvent { item }) = &collected.events[0] else {
        panic!("expected item.completed event");
    };
    let ThreadItemDetails::McpToolCall(item) = &item.details else {
        panic!("expected MCP tool call item");
    };
    let result = item.result.as_ref().expect("expected MCP tool result");
    assert_eq!(
        result.meta,
        Some(json!({"raw_messages": [{"ref_id": "turn0search0"}]}))
    );

    let serialized = serde_json::to_value(&collected.events[0]).expect("serialize event");
    assert_eq!(
        serialized["item"]["result"]["_meta"],
        json!({"raw_messages": [{"ref_id": "turn0search0"}]})
    );
    assert!(serialized["item"]["result"].get("meta").is_none());
}
