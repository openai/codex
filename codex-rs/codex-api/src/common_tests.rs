//! Checks that private nested call IDs and their results share the wire budget.

#![cfg(test)]

use super::MAX_RESPONSE_MESSAGE_BYTES;
use super::with_tool_results;
use codex_protocol::models::ToolResultBody;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;

#[test]
fn nested_call_id_and_result_fit_the_wire_budget_together() {
    let call_id = "private-\"id\\\nλ";
    let result = json!({"content": [{"type": "text", "text": "private result"}]});
    let mcp_result = serde_json::from_value(result.clone()).unwrap();
    let mut body = ToolResultBody::from_mcp_result(&mcp_result).unwrap();
    body.set_nested_call_id(call_id);
    assert!(!format!("{body:?}").contains("private"));
    let bodies = [(0, 0, Arc::new(body))];
    let mut request = json!({"input": [{"internal_chat_message_metadata_passthrough": {
        "executed_tool_calls": [{"name": "search", "arguments": {}}]
    }}]})
    .to_string();
    let enriched = with_tool_results(&request, &bodies).unwrap();
    let wire: Value = serde_json::from_str(&enriched).unwrap();
    assert_eq!(
        wire["input"][0]["internal_chat_message_metadata_passthrough"]["executed_tool_calls"][0],
        json!({"name": "search", "arguments": {}, "tool_result": result, "tool_result_call_id": call_id})
    );

    request.push_str(&" ".repeat(MAX_RESPONSE_MESSAGE_BYTES - enriched.len()));
    assert_eq!(
        with_tool_results(&request, &bodies).unwrap().len(),
        MAX_RESPONSE_MESSAGE_BYTES
    );
    request.push(' ');
    assert!(with_tool_results(&request, &bodies).is_none());
}
