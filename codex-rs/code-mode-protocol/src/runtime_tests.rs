use std::fmt::Debug;

use codex_protocol::ToolName;
use pretty_assertions::assert_eq;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;
use serde_json::json;

use super::*;
use crate::ImageDetail;

fn assert_json_round_trip<T>(value: &T, expected: JsonValue)
where
    T: Debug + DeserializeOwned + PartialEq + Serialize,
{
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    assert_eq!(&serde_json::from_value::<T>(expected).unwrap(), value);
}

fn text(value: &str) -> FunctionCallOutputContentItem {
    FunctionCallOutputContentItem::InputText {
        text: value.to_string(),
    }
}

fn image(image_url: &str, detail: Option<ImageDetail>) -> FunctionCallOutputContentItem {
    FunctionCallOutputContentItem::InputImage {
        image_url: image_url.to_string(),
        detail,
    }
}

#[test]
fn requests_round_trip_with_exact_tool_and_namespace_fields() {
    let create = CreateCellRequest {
        tool_call_id: "response-call-7".to_string(),
        enabled_tools: vec![
            ToolDefinition {
                name: "mcp__search__query".to_string(),
                tool_name: ToolName::namespaced("search", "query"),
                description: "Search indexed documents".to_string(),
                kind: CodeModeToolKind::Function,
                input_schema: Some(json!({
                    "type": "object",
                    "properties": {"q": {"type": "string"}},
                })),
                output_schema: None,
            },
            ToolDefinition {
                name: "apply_patch".to_string(),
                tool_name: ToolName::plain("apply_patch"),
                description: "Apply a patch".to_string(),
                kind: CodeModeToolKind::Freeform,
                input_schema: None,
                output_schema: Some(json!({"type": "string"})),
            },
        ],
        source: "await tools.mcp__search__query({ q: 'rust' });".to_string(),
    };
    assert_json_round_trip(
        &create,
        json!({
            "tool_call_id": "response-call-7",
            "enabled_tools": [
                {
                    "name": "mcp__search__query",
                    "tool_name": {"name": "query", "namespace": "search"},
                    "description": "Search indexed documents",
                    "kind": "function",
                    "input_schema": {
                        "type": "object",
                        "properties": {"q": {"type": "string"}},
                    },
                    "output_schema": null,
                },
                {
                    "name": "apply_patch",
                    "tool_name": {"name": "apply_patch", "namespace": null},
                    "description": "Apply a patch",
                    "kind": "freeform",
                    "input_schema": null,
                    "output_schema": {"type": "string"},
                },
            ],
            "source": "await tools.mcp__search__query({ q: 'rust' });",
        }),
    );

    assert_json_round_trip(
        &ObserveRequest {
            cell_id: CellId::new("cell-a7".to_string()),
            yield_time_ms: 250,
        },
        json!({"cell_id": "cell-a7", "yield_time_ms": 250}),
    );
}

#[test]
fn nested_tool_calls_round_trip_with_optional_input_and_tool_kind() {
    assert_json_round_trip(
        &CodeModeNestedToolCall {
            cell_id: CellId::new("cell-a7".to_string()),
            runtime_tool_call_id: "runtime-call-1".to_string(),
            tool_name: ToolName::namespaced("search", "query"),
            tool_kind: CodeModeToolKind::Function,
            input: Some(json!({"q": "rust"})),
        },
        json!({
            "cell_id": "cell-a7",
            "runtime_tool_call_id": "runtime-call-1",
            "tool_name": {"name": "query", "namespace": "search"},
            "tool_kind": "function",
            "input": {"q": "rust"},
        }),
    );
    assert_json_round_trip(
        &CodeModeNestedToolCall {
            cell_id: CellId::new("cell-b9".to_string()),
            runtime_tool_call_id: "runtime-call-2".to_string(),
            tool_name: ToolName::plain("apply_patch"),
            tool_kind: CodeModeToolKind::Freeform,
            input: None,
        },
        json!({
            "cell_id": "cell-b9",
            "runtime_tool_call_id": "runtime-call-2",
            "tool_name": {"name": "apply_patch", "namespace": null},
            "tool_kind": "freeform",
            "input": null,
        }),
    );
}

#[test]
fn observe_outcomes_use_an_explicit_tagged_wire_shape() {
    assert_json_round_trip(
        &ObserveOutcome::Yielded {
            cell_id: CellId::new("cell-a7".to_string()),
            content_items: vec![
                text("before"),
                image("data:image/png;base64,auto", Some(ImageDetail::Auto)),
                image("data:image/png;base64,low", Some(ImageDetail::Low)),
                image("data:image/png;base64,high", Some(ImageDetail::High)),
                image(
                    "data:image/png;base64,original",
                    Some(ImageDetail::Original),
                ),
                image("data:image/png;base64,default", /*detail*/ None),
            ],
        },
        json!({
            "type": "yielded",
            "cell_id": "cell-a7",
            "content_items": [
                {"type": "input_text", "text": "before"},
                {"type": "input_image", "image_url": "data:image/png;base64,auto", "detail": "auto"},
                {"type": "input_image", "image_url": "data:image/png;base64,low", "detail": "low"},
                {"type": "input_image", "image_url": "data:image/png;base64,high", "detail": "high"},
                {"type": "input_image", "image_url": "data:image/png;base64,original", "detail": "original"},
                {"type": "input_image", "image_url": "data:image/png;base64,default"},
            ],
        }),
    );
    assert_json_round_trip(
        &ObserveOutcome::Completed {
            cell_id: CellId::new("cell-b9".to_string()),
            content_items: vec![text("failed")],
            error_text: Some("tool failed".to_string()),
        },
        json!({
            "type": "completed",
            "cell_id": "cell-b9",
            "content_items": [{"type": "input_text", "text": "failed"}],
            "error_text": "tool failed",
        }),
    );
    assert_json_round_trip(
        &ObserveOutcome::Terminated {
            cell_id: CellId::new("cell-c3".to_string()),
            content_items: vec![text("partial")],
        },
        json!({
            "type": "terminated",
            "cell_id": "cell-c3",
            "content_items": [{"type": "input_text", "text": "partial"}],
        }),
    );
    assert_json_round_trip(
        &ObserveOutcome::Missing {
            cell_id: CellId::new("missing".to_string()),
        },
        json!({"type": "missing", "cell_id": "missing"}),
    );
}

#[test]
fn terminate_outcomes_exclude_yielded_wire_states() {
    assert_json_round_trip(
        &TerminateOutcome::Completed {
            cell_id: CellId::new("cell-a7".to_string()),
            content_items: vec![text("done")],
            error_text: None,
        },
        json!({
            "type": "completed",
            "cell_id": "cell-a7",
            "content_items": [{"type": "input_text", "text": "done"}],
            "error_text": null,
        }),
    );
    assert_json_round_trip(
        &TerminateOutcome::Terminated {
            cell_id: CellId::new("cell-b9".to_string()),
            content_items: vec![text("partial")],
        },
        json!({
            "type": "terminated",
            "cell_id": "cell-b9",
            "content_items": [{"type": "input_text", "text": "partial"}],
        }),
    );
    assert_json_round_trip(
        &TerminateOutcome::Missing {
            cell_id: CellId::new("missing".to_string()),
        },
        json!({"type": "missing", "cell_id": "missing"}),
    );
}
