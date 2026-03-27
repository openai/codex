use codex_api::ResponsesApiRequest;
use codex_api::common::OpenAiVerbosity;
use codex_api::common::TextControls;
use codex_api::create_text_param_for_request;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::models::FunctionCallOutputPayload;
use pretty_assertions::assert_eq;

use super::*;

#[test]
fn serializes_text_verbosity_when_set() {
    let input: Vec<ResponseItem> = vec![];
    let tools: Vec<serde_json::Value> = vec![];
    let req = ResponsesApiRequest {
        model: "gpt-5.1".to_string(),
        instructions: "i".to_string(),
        input,
        tools,
        tool_choice: "auto".to_string(),
        parallel_tool_calls: true,
        reasoning: None,
        store: false,
        stream: true,
        include: vec![],
        prompt_cache_key: None,
        service_tier: None,
        text: Some(TextControls {
            verbosity: Some(OpenAiVerbosity::Low),
            format: None,
        }),
    };

    let v = serde_json::to_value(&req).expect("json");
    assert_eq!(
        v.get("text")
            .and_then(|t| t.get("verbosity"))
            .and_then(|s| s.as_str()),
        Some("low")
    );
}

#[test]
fn serializes_text_schema_with_strict_format() {
    let input: Vec<ResponseItem> = vec![];
    let tools: Vec<serde_json::Value> = vec![];
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "answer": {"type": "string"}
        },
        "required": ["answer"],
    });
    let text_controls =
        create_text_param_for_request(None, &Some(schema.clone())).expect("text controls");

    let req = ResponsesApiRequest {
        model: "gpt-5.1".to_string(),
        instructions: "i".to_string(),
        input,
        tools,
        tool_choice: "auto".to_string(),
        parallel_tool_calls: true,
        reasoning: None,
        store: false,
        stream: true,
        include: vec![],
        prompt_cache_key: None,
        service_tier: None,
        text: Some(text_controls),
    };

    let v = serde_json::to_value(&req).expect("json");
    let text = v.get("text").expect("text field");
    assert!(text.get("verbosity").is_none());
    let format = text.get("format").expect("format field");

    assert_eq!(
        format.get("name"),
        Some(&serde_json::Value::String("codex_output_schema".into()))
    );
    assert_eq!(
        format.get("type"),
        Some(&serde_json::Value::String("json_schema".into()))
    );
    assert_eq!(format.get("strict"), Some(&serde_json::Value::Bool(true)));
    assert_eq!(format.get("schema"), Some(&schema));
}

#[test]
fn omits_text_when_not_set() {
    let input: Vec<ResponseItem> = vec![];
    let tools: Vec<serde_json::Value> = vec![];
    let req = ResponsesApiRequest {
        model: "gpt-5.1".to_string(),
        instructions: "i".to_string(),
        input,
        tools,
        tool_choice: "auto".to_string(),
        parallel_tool_calls: true,
        reasoning: None,
        store: false,
        stream: true,
        include: vec![],
        prompt_cache_key: None,
        service_tier: None,
        text: None,
    };

    let v = serde_json::to_value(&req).expect("json");
    assert!(v.get("text").is_none());
}

#[test]
fn serializes_flex_service_tier_when_set() {
    let req = ResponsesApiRequest {
        model: "gpt-5.1".to_string(),
        instructions: "i".to_string(),
        input: vec![],
        tools: vec![],
        tool_choice: "auto".to_string(),
        parallel_tool_calls: true,
        reasoning: None,
        store: false,
        stream: true,
        include: vec![],
        prompt_cache_key: None,
        service_tier: Some(ServiceTier::Flex.to_string()),
        text: None,
    };

    let v = serde_json::to_value(&req).expect("json");
    assert_eq!(
        v.get("service_tier").and_then(|tier| tier.as_str()),
        Some("flex")
    );
}

#[test]
fn reserializes_shell_outputs_for_function_and_custom_tool_calls() {
    let raw_output = r#"{"output":"hello","metadata":{"exit_code":0,"duration_seconds":0.5}}"#;
    let expected_output = "Exit code: 0\nWall time: 0.5 seconds\nOutput:\nhello";
    let mut items = vec![
        ResponseItem::FunctionCall {
            id: None,
            name: "shell".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: "call-1".to_string(),
        },
        ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_text(raw_output.to_string()),
        },
        ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "call-2".to_string(),
            name: "apply_patch".to_string(),
            input: "*** Begin Patch".to_string(),
        },
        ResponseItem::CustomToolCallOutput {
            call_id: "call-2".to_string(),
            name: None,
            output: FunctionCallOutputPayload::from_text(raw_output.to_string()),
        },
    ];

    reserialize_shell_outputs(&mut items);

    assert_eq!(
        items,
        vec![
            ResponseItem::FunctionCall {
                id: None,
                name: "shell".to_string(),
                namespace: None,
                arguments: "{}".to_string(),
                call_id: "call-1".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call-1".to_string(),
                output: FunctionCallOutputPayload::from_text(expected_output.to_string()),
            },
            ResponseItem::CustomToolCall {
                id: None,
                status: None,
                call_id: "call-2".to_string(),
                name: "apply_patch".to_string(),
                input: "*** Begin Patch".to_string(),
            },
            ResponseItem::CustomToolCallOutput {
                call_id: "call-2".to_string(),
                name: None,
                output: FunctionCallOutputPayload::from_text(expected_output.to_string()),
            },
        ]
    );
}
