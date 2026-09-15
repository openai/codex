use super::*;

fn computer_item(
    id: &str,
    status: codex_app_server_protocol::McpToolCallStatus,
) -> AppServerThreadItem {
    AppServerThreadItem::McpToolCall {
        id: id.to_string(),
        server: "cua_repl".to_string(),
        tool: "js".to_string(),
        status,
        arguments: json!({"title": format!("Inspect page {id}"), "code": "await cua.getState()"}),
        app_context: None,
        mcp_app_resource_uri: None,
        mcp_app_ui: None,
        plugin_id: None,
        read_only_hint: None,
        result: Some(Box::new(codex_app_server_protocol::McpToolCallResult {
            content: vec![json!({"type": "text", "text": format!("Full output for {id}")})],
            structured_content: None,
            meta: None,
        })),
        error: None,
        duration_ms: Some(5),
    }
}

#[tokio::test]
async fn computer_activity_live_and_replay_group_identically() {
    use codex_app_server_protocol::McpToolCallStatus;
    let mut outputs = Vec::new();
    for replay in [false, true] {
        let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        for id in ["1", "2", "3", "4"] {
            let item = computer_item(id, McpToolCallStatus::Completed);
            if replay {
                chat.replay_thread_item(item, "turn-1".to_string(), ReplayKind::ThreadSnapshot);
            } else {
                chat.on_mcp_tool_call_started(computer_item(id, McpToolCallStatus::InProgress));
                chat.on_mcp_tool_call_completed(item);
            }
        }
        assert!(drain_insert_history(&mut rx).is_empty());
        let cell = chat.transcript.active_cell.as_ref().expect("group");
        outputs.push((
            cell.display_lines(/*width*/ 80),
            cell.transcript_lines(/*width*/ 100),
        ));
        chat.finalize_turn();
        let cells = drain_insert_history(&mut rx);
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0], outputs.last().unwrap().0);
    }
    assert_eq!(outputs[0], outputs[1]);
    insta::assert_snapshot!(
        "computer_activity_replayed",
        lines_to_single_string(&outputs[0].0)
    );
}

#[tokio::test]
async fn computer_activity_preserves_boundaries_and_expanded_output() {
    use codex_app_server_protocol::McpToolCallStatus;
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_mcp_tool_call_completed(computer_item("1", McpToolCallStatus::Completed));
    chat.prepare_assistant_message();
    let cells = drain_insert_history(&mut rx);
    assert!(lines_to_single_string(&cells[0]).contains("Used computer · 1 action"));
    chat.on_mcp_tool_call_completed(computer_item("2", McpToolCallStatus::Completed));
    let mut other = computer_item("other", McpToolCallStatus::Completed);
    if let AppServerThreadItem::McpToolCall { server, .. } = &mut other {
        *server = "another_server".to_string();
    }
    chat.on_mcp_tool_call_completed(other);
    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 2);
    assert!(lines_to_single_string(&cells[0]).contains("Used computer · 1 action"));
    assert!(lines_to_single_string(&cells[1]).contains("another_server"));
    chat.on_mcp_tool_call_completed(computer_item("3", McpToolCallStatus::Completed));
    let transcript = chat.active_cell_transcript_lines(/*width*/ 100).unwrap();
    assert!(lines_to_single_string(&transcript).contains("Full output for 3"));
    assert!(lines_to_single_string(&transcript).contains("await cua.getState()"));
}

#[tokio::test]
async fn computer_activity_out_of_order_completion_and_interruption() {
    use codex_app_server_protocol::McpToolCallStatus;
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.on_mcp_tool_call_started(computer_item("1", McpToolCallStatus::InProgress));
    chat.on_mcp_tool_call_started(computer_item("2", McpToolCallStatus::InProgress));
    chat.on_mcp_tool_call_completed(computer_item("2", McpToolCallStatus::Completed));
    assert!(active_blob(&chat).contains("Using computer"));
    chat.finalize_turn();
    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1);
    insta::assert_snapshot!(
        "computer_activity_partial_interruption",
        lines_to_single_string(&cells[0])
    );
}
