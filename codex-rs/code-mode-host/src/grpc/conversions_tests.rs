use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::ImageDetail;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::grpc as proto;
use pretty_assertions::assert_eq;
use tonic::Code;

use super::execute_request;
use super::execution_outcome;

#[test]
fn rejects_missing_names_unknown_tool_kinds_and_invalid_json_schemas() {
    let definition = proto::ToolDefinition {
        name: "echo".to_string(),
        tool_name: None,
        description: String::new(),
        kind: proto::ToolKind::Function as i32,
        input_schema_json: None,
        output_schema_json: None,
    };
    let request = |definition| proto::ExecuteRequest {
        session_id: "session".to_string(),
        execution_id: "execution".to_string(),
        tool_call_id: "call".to_string(),
        source: String::new(),
        enabled_tools: vec![definition],
        yield_time_ms: None,
        max_output_tokens: None,
    };

    assert_eq!(
        execute_request(request(definition.clone()))
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    let definition = proto::ToolDefinition {
        tool_name: Some(proto::ToolName {
            name: "echo".to_string(),
            namespace: Some("tools".to_string()),
        }),
        ..definition
    };
    assert_eq!(
        execute_request(request(proto::ToolDefinition {
            kind: proto::ToolKind::Unspecified as i32,
            ..definition.clone()
        }))
        .unwrap_err()
        .code(),
        Code::InvalidArgument
    );
    assert_eq!(
        execute_request(request(proto::ToolDefinition {
            input_schema_json: Some(b"not-json".to_vec()),
            ..definition
        }))
        .unwrap_err()
        .code(),
        Code::InvalidArgument
    );
}

#[test]
fn maps_text_image_audio_and_terminal_error_without_losing_details() {
    let outcome = execution_outcome(RuntimeResponse::Result {
        cell_id: CellId::new("cell".to_string()),
        content_items: vec![
            FunctionCallOutputContentItem::InputText {
                text: "hello".to_string(),
            },
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,YQ==".to_string(),
                detail: Some(ImageDetail::Original),
            },
            FunctionCallOutputContentItem::InputAudio {
                audio_url: "data:audio/wav;base64,YQ==".to_string(),
            },
        ],
        error_text: Some("failed".to_string()),
    });

    assert_eq!(
        outcome,
        proto::ExecutionOutcome {
            cell_id: "cell".to_string(),
            content_items: vec![
                proto::ContentItem {
                    item: Some(proto::content_item::Item::Text(proto::TextContent {
                        text: "hello".to_string(),
                    })),
                },
                proto::ContentItem {
                    item: Some(proto::content_item::Item::Image(proto::ImageContent {
                        image_url: "data:image/png;base64,YQ==".to_string(),
                        detail: Some(proto::ImageDetail::Original as i32),
                    })),
                },
                proto::ContentItem {
                    item: Some(proto::content_item::Item::Audio(proto::AudioContent {
                        audio_url: "data:audio/wav;base64,YQ==".to_string(),
                    })),
                },
            ],
            outcome: Some(proto::execution_outcome::Outcome::Completed(
                proto::ExecutionCompleted {
                    error_text: Some("failed".to_string()),
                },
            )),
        }
    );
}
