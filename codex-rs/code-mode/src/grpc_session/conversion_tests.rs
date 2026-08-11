use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeToolKind;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::ImageDetail;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::ToolDefinition;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::grpc;
use codex_protocol::ToolName;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::execute_request;
use super::runtime_response;
use super::tool_call;
use super::wait_outcome;

#[test]
fn execute_request_preserves_tool_schemas_namespaces_and_limits() {
    let request = ExecuteRequest {
        tool_call_id: "outer".to_string(),
        enabled_tools: vec![ToolDefinition {
            name: "search".to_string(),
            tool_name: ToolName::namespaced("work", "search"),
            description: "search the workspace".to_string(),
            kind: CodeModeToolKind::Freeform,
            input_schema: Some(json!({"type": "object"})),
            output_schema: Some(json!({"type": "string"})),
        }],
        source: "text('hello')".to_string(),
        yield_time_ms: Some(25),
        max_output_tokens: Some(128),
    };

    assert_eq!(
        execute_request("session", "execution".to_string(), request),
        Ok(grpc::ExecuteRequest {
            session_id: "session".to_string(),
            execution_id: "execution".to_string(),
            tool_call_id: "outer".to_string(),
            source: "text('hello')".to_string(),
            enabled_tools: vec![grpc::ToolDefinition {
                name: "search".to_string(),
                tool_name: Some(grpc::ToolName {
                    name: "search".to_string(),
                    namespace: Some("work".to_string()),
                }),
                description: "search the workspace".to_string(),
                kind: grpc::ToolKind::Freeform as i32,
                input_schema_json: Some(br#"{"type":"object"}"#.to_vec()),
                output_schema_json: Some(br#"{"type":"string"}"#.to_vec()),
            }],
            yield_time_ms: Some(25),
            max_output_tokens: Some(128),
        })
    );
}

#[test]
fn tool_call_decodes_structured_input_and_namespace() {
    let call = grpc::ToolCall {
        session_id: "session".to_string(),
        execution_id: "execution".to_string(),
        cell_id: "cell".to_string(),
        invocation_id: "invocation".to_string(),
        runtime_tool_call_id: "runtime-call".to_string(),
        tool_name: Some(grpc::ToolName {
            name: "search".to_string(),
            namespace: Some("work".to_string()),
        }),
        tool_kind: grpc::ToolKind::Function as i32,
        input_json: Some(br#"{"query":"hello"}"#.to_vec()),
        sequence: 1,
    };

    assert_eq!(
        tool_call(call),
        Ok(CodeModeNestedToolCall {
            cell_id: CellId::new("cell".to_string()),
            runtime_tool_call_id: "runtime-call".to_string(),
            tool_name: ToolName::namespaced("work", "search"),
            tool_kind: CodeModeToolKind::Function,
            input: Some(json!({"query": "hello"})),
        })
    );
}

#[test]
fn runtime_response_decodes_mixed_content_items() {
    let outcome = grpc::ExecutionOutcome {
        cell_id: "cell".to_string(),
        content_items: vec![
            grpc::ContentItem {
                item: Some(grpc::content_item::Item::Text(grpc::TextContent {
                    text: "hello".to_string(),
                })),
            },
            grpc::ContentItem {
                item: Some(grpc::content_item::Item::Image(grpc::ImageContent {
                    image_url: "data:image/png;base64,AA==".to_string(),
                    detail: Some(grpc::ImageDetail::Original as i32),
                })),
            },
            grpc::ContentItem {
                item: Some(grpc::content_item::Item::Audio(grpc::AudioContent {
                    audio_url: "data:audio/wav;base64,AA==".to_string(),
                })),
            },
        ],
        outcome: Some(grpc::execution_outcome::Outcome::Completed(
            grpc::ExecutionCompleted {
                error_text: Some("warning".to_string()),
            },
        )),
    };

    assert_eq!(
        runtime_response(outcome),
        Ok(RuntimeResponse::Result {
            cell_id: CellId::new("cell".to_string()),
            content_items: vec![
                FunctionCallOutputContentItem::InputText {
                    text: "hello".to_string(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,AA==".to_string(),
                    detail: Some(ImageDetail::Original),
                },
                FunctionCallOutputContentItem::InputAudio {
                    audio_url: "data:audio/wav;base64,AA==".to_string(),
                },
            ],
            error_text: Some("warning".to_string()),
        })
    );
}

#[test]
fn wait_outcome_preserves_missing_cell_state() {
    let response = grpc::WaitResponse {
        state: Some(grpc::wait_response::State::MissingCell(
            grpc::ExecutionOutcome {
                cell_id: "missing".to_string(),
                content_items: Vec::new(),
                outcome: Some(grpc::execution_outcome::Outcome::Terminated(
                    grpc::ExecutionTerminated {},
                )),
            },
        )),
    };

    assert_eq!(
        wait_outcome(response),
        Ok(WaitOutcome::MissingCell(RuntimeResponse::Terminated {
            cell_id: CellId::new("missing".to_string()),
            content_items: Vec::new(),
        }))
    );
}

#[test]
fn oversized_response_cell_ids_are_rejected() {
    let response = grpc::ExecutionOutcome {
        cell_id: "x".repeat(grpc::MAX_IDENTIFIER_BYTES + 1),
        content_items: Vec::new(),
        outcome: Some(grpc::execution_outcome::Outcome::Yielded(
            grpc::ExecutionYielded {},
        )),
    };

    assert_eq!(
        runtime_response(response),
        Err(format!(
            "gRPC code-mode host returned cell ID exceeding {} bytes",
            grpc::MAX_IDENTIFIER_BYTES
        ))
    );
}

#[test]
fn invalid_output_enums_and_missing_oneofs_are_rejected() {
    let invalid_image = grpc::ExecutionOutcome {
        cell_id: "cell".to_string(),
        content_items: vec![grpc::ContentItem {
            item: Some(grpc::content_item::Item::Image(grpc::ImageContent {
                image_url: "image".to_string(),
                detail: Some(grpc::ImageDetail::Unspecified as i32),
            })),
        }],
        outcome: Some(grpc::execution_outcome::Outcome::Yielded(
            grpc::ExecutionYielded {},
        )),
    };

    assert!(runtime_response(invalid_image).is_err());
    assert!(wait_outcome(grpc::WaitResponse { state: None }).is_err());
}
