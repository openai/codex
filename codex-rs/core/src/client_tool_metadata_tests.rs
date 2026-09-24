use super::*;
use codex_api::ResponseCreateWsRequest;
use codex_api::ResponsesApiRequest;
use codex_api::ResponsesWsRequest;
use codex_protocol::models::ExecutedToolCall;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ToolResultMetadata;
use codex_protocol::models::ToolResultSource;
use codex_protocol::models::ToolResultSources;
use pretty_assertions::assert_eq;
use serde_json::json;

fn request() -> ResponsesApiRequest {
    request_with_metadata(&json!({
        "provider": "x".repeat(16 * 1024),
        "openai/resource_access": {"resources": ["é".repeat(4 * 1024)]},
    }))
}

fn request_with_metadata(metadata: &serde_json::Value) -> ResponsesApiRequest {
    let mut output = ResponseItem::from(ResponseInputItem::FunctionCallOutput {
        call_id: "call".to_string(),
        output: FunctionCallOutputPayload::from_text("keep result".to_string()),
    });
    let mut call = ExecutedToolCall::new("mcp__apps__read".to_string(), json!({"query": "keep"}));
    call.set_tool_result_metadata(ToolResultMetadata::new(metadata));
    call.set_tool_result_sources(ToolResultSources::new(vec![ToolResultSource {
        r#type: "test_resource".to_string(),
        id: "resource".to_string(),
    }]));
    output.append_executed_tool_calls(vec![call]);
    output.set_tool_call_cell_id("cell");
    output.mark_tool_calls_complete();
    ResponsesApiRequest {
        model: "test".to_string(),
        instructions: "x".to_string(),
        input: vec![output],
        tools: None,
        tool_choice: "auto".to_string(),
        parallel_tool_calls: true,
        reasoning: None,
        store: false,
        stream: true,
        stream_options: None,
        include: Vec::new(),
        service_tier: None,
        prompt_cache_key: None,
        text: None,
        client_metadata: None,
        access_programs: None,
    }
}

#[test]
fn http_message_budget_preserves_resources_until_the_message_still_exceeds_limit() {
    for overage in [0, 4 * 1024, 20 * 1024, 40 * 1024] {
        let mut request = request();
        let target_bytes = MAX_RESPONSE_MESSAGE_BYTES + overage;
        request.instructions =
            "x".repeat(target_bytes - serialized_json_bytes(&request).unwrap() + 1);
        assert_eq!(serialized_json_bytes(&request).unwrap(), target_bytes);
        assert!(metadata_metrics::metadata_bytes(&request.input) < 32 * 1024);
        let original_input = request.input.clone();
        let bounded = bounded_input(&request, &request.input);
        if overage == 0 {
            assert_eq!(bounded, None);
            continue;
        }
        let bounded = bounded.unwrap();
        let encoded = serde_json::to_value(&bounded[0]).unwrap();
        let metadata = &encoded["internal_chat_message_metadata_passthrough"]["executed_tool_calls"]
            [0]["tool_result_metadata"];
        if overage == 4 * 1024 {
            assert_eq!(
                metadata,
                &json!({
                    "openai/resource_access": {"resources": ["é".repeat(4 * 1024)]},
                })
            );
        } else if overage == 20 * 1024 {
            let mut resource_only = original_input.clone();
            resource_only[0].retain_tool_resource_access();
            let first_reduction = metadata_metrics::metadata_bytes(&original_input)
                - metadata_metrics::metadata_bytes(&resource_only);
            assert_eq!(
                metadata,
                &json!(format!(
                    "omitted_due_to_size_limit (overage_bytes={})",
                    overage - first_reduction,
                ))
            );
        } else {
            assert!(
                encoded["internal_chat_message_metadata_passthrough"]["executed_tool_calls"][0]
                    .get("tool_result_metadata")
                    .is_none()
            );
        }
        assert_eq!(request.input, original_input);
        let mut ordinary = original_input;
        let mut bounded_ordinary = bounded.clone();
        for items in [&mut ordinary, &mut bounded_ordinary] {
            for item in items {
                item.clear_tool_result_metadata();
            }
        }
        assert_eq!(bounded_ordinary, ordinary);
        request.input = bounded;
        // The last case cannot fit even without raw metadata: ordinary content is never cut.
        assert_eq!(
            serialized_json_bytes(&request).unwrap() > MAX_RESPONSE_MESSAGE_BYTES,
            overage == 40 * 1024,
        );
        assert_eq!(bounded_input(&request, &request.input), None);
    }
}

#[test]
fn message_budget_removes_residual_values_and_markers_when_ordinary_input_fits() {
    for metadata in [
        json!({}),
        json!(null),
        json!("omitted_due_to_size_limit"),
        json!("omitted_due_to_size_limit (overage_bytes=1)"),
        json!({"provider": "x".repeat(1024)}),
        json!({"openai/resource_access": {}}),
    ] {
        for websocket in [false, true] {
            let mut request = request_with_metadata(&metadata);
            let mut ordinary = request.clone();
            ordinary.input[0].clear_tool_result_metadata();
            let ordinary_bytes = if websocket {
                serialized_json_bytes(&ResponsesWsRequest::ResponseCreate(
                    ResponseCreateWsRequest::from(&ordinary),
                ))
                .unwrap()
            } else {
                serialized_json_bytes(&ordinary).unwrap()
            };
            // The original instructions contain one byte, so leave exactly one
            // byte below the limit after removing only result metadata.
            ordinary.instructions = "x".repeat(MAX_RESPONSE_MESSAGE_BYTES - ordinary_bytes);
            request.instructions = ordinary.instructions.clone();
            let original_input = request.input.clone();
            let bounded = if websocket {
                let ordinary_message =
                    ResponsesWsRequest::ResponseCreate(ResponseCreateWsRequest::from(&ordinary));
                assert_eq!(
                    serialized_json_bytes(&ordinary_message).unwrap(),
                    MAX_RESPONSE_MESSAGE_BYTES - 1,
                );
                let mut message =
                    ResponsesWsRequest::ResponseCreate(ResponseCreateWsRequest::from(&request));
                assert!(serialized_json_bytes(&message).unwrap() > MAX_RESPONSE_MESSAGE_BYTES);
                let bounded = bounded_input(&message, &request.input)
                    .expect("optional result metadata must not keep a fitting message oversized");
                let ResponsesWsRequest::ResponseCreate(payload) = &mut message;
                payload.input = &bounded;
                assert_eq!(
                    serialized_json_bytes(&message).unwrap(),
                    MAX_RESPONSE_MESSAGE_BYTES - 1,
                );
                bounded
            } else {
                assert_eq!(
                    serialized_json_bytes(&ordinary).unwrap(),
                    MAX_RESPONSE_MESSAGE_BYTES - 1,
                );
                assert!(serialized_json_bytes(&request).unwrap() > MAX_RESPONSE_MESSAGE_BYTES);
                let bounded = bounded_input(&request, &request.input)
                    .expect("optional result metadata must not keep a fitting message oversized");
                let mut message = request.clone();
                message.input = bounded.clone();
                assert_eq!(message, ordinary);
                assert_eq!(
                    serialized_json_bytes(&message).unwrap(),
                    MAX_RESPONSE_MESSAGE_BYTES - 1,
                );
                bounded
            };
            // Whole-input equality covers sources, calls, completion and ordinary output.
            assert_eq!(bounded, ordinary.input);
            assert_eq!(request.input, original_input);
        }
    }
}

#[test]
fn residual_marker_removal_preserves_earlier_resources_and_stops_when_message_fits() {
    let mut request = request_with_metadata(&json!({
        "openai/resource_access": {
            "schema_version": 1,
            "resource_coverage": "complete",
            "resources": [],
        },
    }));
    for call_id in ["marker", "later-marker"] {
        let mut output = ResponseItem::from(ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: FunctionCallOutputPayload::from_text("keep result".to_string()),
        });
        let mut call = ExecutedToolCall::new("mcp__apps__read".to_string(), json!({}));
        call.set_tool_result_metadata(ToolResultMetadata::new(&json!(
            "omitted_due_to_size_limit (overage_bytes=1)"
        )));
        output.append_executed_tool_calls(vec![call]);
        output.mark_tool_calls_complete();
        request.input.push(output);
    }
    let mut expected = request.clone();
    expected.input[1].clear_tool_result_metadata();
    expected.instructions =
        "x".repeat(MAX_RESPONSE_MESSAGE_BYTES - serialized_json_bytes(&expected).unwrap() + 1);
    request.instructions = expected.instructions.clone();
    assert_eq!(
        serialized_json_bytes(&expected).unwrap(),
        MAX_RESPONSE_MESSAGE_BYTES
    );
    assert!(serialized_json_bytes(&request).unwrap() > MAX_RESPONSE_MESSAGE_BYTES);
    let original_input = request.input.clone();
    let bounded = bounded_input(&request, &request.input).unwrap();
    assert_eq!(bounded, expected.input);
    assert_eq!(request.input, original_input);
    request.input = bounded;
    assert_eq!(request, expected);
    assert_eq!(
        serialized_json_bytes(&request).unwrap(),
        MAX_RESPONSE_MESSAGE_BYTES
    );
}

#[test]
fn websocket_budget_uses_the_actual_delta_and_does_not_mutate_logical_history() {
    let mut request = request();
    let old_output = ResponseItem::from(ResponseInputItem::FunctionCallOutput {
        call_id: "old".to_string(),
        output: FunctionCallOutputPayload::from_text("x".repeat(MAX_RESPONSE_MESSAGE_BYTES)),
    });
    request.input.insert(0, old_output);
    let delta = &request.input[1..];
    let delta_message = ResponsesWsRequest::ResponseCreate(ResponseCreateWsRequest {
        previous_response_id: Some("previous-response".to_string()),
        input: delta,
        ..ResponseCreateWsRequest::from(&request)
    });
    assert!(serialized_json_bytes(&request).unwrap() > MAX_RESPONSE_MESSAGE_BYTES);
    assert!(serialized_json_bytes(&delta_message).unwrap() < 32 * 1024);
    assert_eq!(bounded_input(&delta_message, delta), None);

    let mut full = ResponsesWsRequest::ResponseCreate(ResponseCreateWsRequest::from(&request));
    let bounded = bounded_input(&full, &request.input).unwrap();
    let ResponsesWsRequest::ResponseCreate(payload) = &mut full;
    payload.input = &bounded;
    assert_eq!(bounded[0], request.input[0]);
    let mut ordinary = request.input[1].clone();
    let mut bounded_ordinary = bounded[1].clone();
    ordinary.clear_tool_result_metadata();
    bounded_ordinary.clear_tool_result_metadata();
    assert_eq!(bounded_ordinary, ordinary);
    assert_eq!(request.input[1], self::request().input[0]);
    assert_eq!(bounded_input(&full, &bounded), None);
}
