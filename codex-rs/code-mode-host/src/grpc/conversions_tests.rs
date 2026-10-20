use std::time::Duration;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::ImageDetail;
use codex_code_mode_protocol::MissingCodeModeHostDuration;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::grpc as proto;
use pretty_assertions::assert_eq;
use tonic::Code;

use super::execute_request;
use super::execution_outcome;
use super::wait_response;

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
    let response = wait_response(WaitOutcome::LiveCell(RuntimeResponse::Result {
        code_mode_host_duration: Some(Duration::from_nanos(/*nanos*/ 123_456_789)),
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
        result_buffered_for: Duration::from_nanos(42),
        error_text: Some("failed".to_string()),
    }))
    .unwrap();

    assert_eq!(
        response
            .metadata()
            .get(proto::RESULT_BUFFERED_FOR_NS_METADATA)
            .unwrap(),
        "42"
    );
    let state = response.into_inner().state.expect("wait outcome");
    assert_eq!(
        state,
        proto::wait_response::State::LiveCell(proto::ExecutionOutcome {
            code_mode_host_duration_ns: 123_456_789,
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
        })
    );
}

#[test]
fn wait_metadata_distinguishes_an_unbuffered_result_from_no_result() {
    let result = RuntimeResponse::Result {
        cell_id: CellId::new("cell".to_string()),
        content_items: Vec::new(),
        error_text: None,
        result_buffered_for: Duration::ZERO,
        code_mode_host_duration: Some(Duration::ZERO),
    };
    for (outcome, expected) in [
        (WaitOutcome::LiveCell(result.clone()), Some("0")),
        (WaitOutcome::MissingCell(result), None),
    ] {
        let response = wait_response(outcome).unwrap();
        let buffered = response
            .metadata()
            .get(proto::RESULT_BUFFERED_FOR_NS_METADATA)
            .map(|value| value.to_str().unwrap());
        assert_eq!(buffered, expected);
    }
}

/// Encoding must not turn a missing request measurement into measured zero,
/// including when no live cell remains to supply output.
#[test]
fn grpc_encoding_rejects_untimed_runtime_output() {
    let response = RuntimeResponse::Terminated {
        cell_id: CellId::new("cell".to_string()),
        content_items: Vec::new(),
        code_mode_host_duration: None,
    };
    assert_eq!(
        execution_outcome(response.clone()),
        Err(MissingCodeModeHostDuration)
    );
    for outcome in [
        WaitOutcome::LiveCell(response.clone()),
        WaitOutcome::MissingCell(response),
    ] {
        assert_eq!(
            wait_response(outcome).unwrap_err(),
            MissingCodeModeHostDuration
        );
    }
}
