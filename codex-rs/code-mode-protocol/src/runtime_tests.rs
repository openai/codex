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
    assert_json_round_trip(
        &ObserveToPendingRequest {
            cell_id: CellId::new("cell-b9".to_string()),
        },
        json!({"cell_id": "cell-b9"}),
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
fn runtime_responses_round_trip_with_every_output_and_terminal_variant() {
    let yielded = RuntimeResponse::Yielded {
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
            image("data:image/png;base64,default", None),
        ],
    };
    assert_json_round_trip(
        &yielded,
        json!({
            "Yielded": {
                "cell_id": "cell-a7",
                "content_items": [
                    {"type": "input_text", "text": "before"},
                    {"type": "input_image", "image_url": "data:image/png;base64,auto", "detail": "auto"},
                    {"type": "input_image", "image_url": "data:image/png;base64,low", "detail": "low"},
                    {"type": "input_image", "image_url": "data:image/png;base64,high", "detail": "high"},
                    {"type": "input_image", "image_url": "data:image/png;base64,original", "detail": "original"},
                    {"type": "input_image", "image_url": "data:image/png;base64,default"},
                ],
            },
        }),
    );
    assert_json_round_trip(
        &RuntimeResponse::Terminated {
            cell_id: CellId::new("cell-b9".to_string()),
            content_items: vec![text("partial")],
        },
        json!({
            "Terminated": {
                "cell_id": "cell-b9",
                "content_items": [{"type": "input_text", "text": "partial"}],
            },
        }),
    );
    assert_json_round_trip(
        &RuntimeResponse::Result {
            cell_id: CellId::new("cell-c3".to_string()),
            content_items: vec![text("failed")],
            error_text: Some("tool failed".to_string()),
        },
        json!({
            "Result": {
                "cell_id": "cell-c3",
                "content_items": [{"type": "input_text", "text": "failed"}],
                "error_text": "tool failed",
            },
        }),
    );
    assert_json_round_trip(
        &RuntimeResponse::Result {
            cell_id: CellId::new("cell-d4".to_string()),
            content_items: Vec::new(),
            error_text: None,
        },
        json!({
            "Result": {
                "cell_id": "cell-d4",
                "content_items": [],
                "error_text": null,
            },
        }),
    );
}

#[test]
fn observation_outcomes_round_trip_with_live_missing_pending_and_completed_cells() {
    let result = RuntimeResponse::Result {
        cell_id: CellId::new("cell-a7".to_string()),
        content_items: vec![text("done")],
        error_text: None,
    };
    assert_json_round_trip(
        &CellOutcome::LiveCell(result.clone()),
        json!({
            "LiveCell": {
                "Result": {
                    "cell_id": "cell-a7",
                    "content_items": [{"type": "input_text", "text": "done"}],
                    "error_text": null,
                },
            },
        }),
    );
    assert_json_round_trip(
        &CellOutcome::MissingCell(RuntimeResponse::Terminated {
            cell_id: CellId::new("missing".to_string()),
            content_items: Vec::new(),
        }),
        json!({
            "MissingCell": {
                "Terminated": {"cell_id": "missing", "content_items": []},
            },
        }),
    );

    let pending = PendingOutcome::Pending {
        cell_id: CellId::new("cell-b9".to_string()),
        content_items: vec![text("waiting")],
        pending_tool_call_ids: vec!["runtime-call-1".to_string()],
    };
    assert_json_round_trip(
        &pending,
        json!({
            "Pending": {
                "cell_id": "cell-b9",
                "content_items": [{"type": "input_text", "text": "waiting"}],
                "pending_tool_call_ids": ["runtime-call-1"],
            },
        }),
    );
    assert_json_round_trip(
        &PendingOutcome::Completed(result),
        json!({
            "Completed": {
                "Result": {
                    "cell_id": "cell-a7",
                    "content_items": [{"type": "input_text", "text": "done"}],
                    "error_text": null,
                },
            },
        }),
    );
    assert_json_round_trip(
        &ObserveToPendingOutcome::LiveCell(pending),
        json!({
            "LiveCell": {
                "Pending": {
                    "cell_id": "cell-b9",
                    "content_items": [{"type": "input_text", "text": "waiting"}],
                    "pending_tool_call_ids": ["runtime-call-1"],
                },
            },
        }),
    );
    assert_json_round_trip(
        &ObserveToPendingOutcome::MissingCell(RuntimeResponse::Result {
            cell_id: CellId::new("missing".to_string()),
            content_items: Vec::new(),
            error_text: Some("cell not found".to_string()),
        }),
        json!({
            "MissingCell": {
                "Result": {
                    "cell_id": "missing",
                    "content_items": [],
                    "error_text": "cell not found",
                },
            },
        }),
    );
}
