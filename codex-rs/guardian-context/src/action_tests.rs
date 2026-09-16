//! Approval projections omit optional host metadata without changing the actual action.

use super::action_for_review;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn mcp_projection_preserves_arguments_with_description_fields() {
    let arguments = json!({
        "description": "required argument",
        "tool_description": "also an argument",
        "nested": {"connector_description": "required nested argument"},
    });
    let action = json!({
        "tool": "mcp_tool_call",
        "server": "example",
        "tool_name": "publish",
        "tool_description": "optional host metadata",
        "connector_description": "optional connector metadata",
        "arguments": arguments,
    });
    assert_eq!(
        action_for_review(action),
        json!({
            "tool": "mcp_tool_call",
            "server": "example",
            "tool_name": "publish",
            "arguments": arguments,
        }),
    );
}

#[test]
fn other_actions_preserve_description_fields() {
    let action = json!({
        "tool": "custom_action",
        "tool_description": "required argument",
        "connector_description": "also required",
    });
    assert_eq!(action_for_review(action.clone()), action);
}
