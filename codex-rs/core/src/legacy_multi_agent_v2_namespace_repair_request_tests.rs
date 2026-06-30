use super::ModelClient;
use crate::Prompt;
use crate::test_support::TestCodexResponsesRequestKind;
use crate::test_support::responses_metadata;
use codex_model_provider_info::WireApi;
use codex_model_provider_info::create_oss_provider_with_base_url;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::SessionSource;
use codex_tools::JsonSchema;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use pretty_assertions::assert_eq;
use serde_json::json;

const CUSTOM_NAMESPACE: &str = "agents";
const MULTI_AGENT_V2_FUNCTION_NAMES: [&str; 6] = [
    "spawn_agent",
    "send_message",
    "followup_task",
    "wait_agent",
    "interrupt_agent",
    "list_agents",
];

fn function(function_name: &str) -> ResponsesApiTool {
    ResponsesApiTool {
        name: function_name.to_string(),
        description: format!("{function_name} description"),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::default(),
        output_schema: None,
    }
}

fn flat_function(function_name: &str) -> ToolSpec {
    ToolSpec::Function(function(function_name))
}

fn namespace(namespace_name: &str, function_names: &[&str]) -> ToolSpec {
    ToolSpec::Namespace(ResponsesApiNamespace {
        name: namespace_name.to_string(),
        description: format!("{namespace_name} description"),
        tools: function_names
            .iter()
            .map(|function_name| ResponsesApiNamespaceTool::Function(function(function_name)))
            .collect(),
    })
}

fn multi_agent_v2_namespace(namespace_name: &str) -> ToolSpec {
    namespace(namespace_name, &MULTI_AGENT_V2_FUNCTION_NAMES)
}

fn function_call(function_name: &str, namespace: Option<&str>) -> ResponseItem {
    ResponseItem::FunctionCall {
        id: Some(format!("{function_name}-item")),
        name: function_name.to_string(),
        namespace: namespace.map(str::to_string),
        arguments: format!(r#"{{"function":"{function_name}"}}"#),
        call_id: format!("{function_name}-call"),
        internal_chat_message_metadata_passthrough: None,
    }
}

fn test_model_client() -> ModelClient {
    let provider_info =
        create_oss_provider_with_base_url("https://example.com/v1", WireApi::Responses);
    ModelClient::new(
        /*auth_manager*/ None,
        ThreadId::new(),
        provider_info,
        SessionSource::Cli,
        "test_originator".to_string(),
        /*model_verbosity*/ None,
        /*enable_request_compression*/ false,
        /*include_timing_metrics*/ false,
        /*beta_features_header*/ None,
        /*item_ids_enabled*/ false,
        /*attestation_provider*/ None,
    )
}

fn test_model_info() -> ModelInfo {
    serde_json::from_value(json!({
        "slug": "gpt-test",
        "display_name": "gpt-test",
        "description": "desc",
        "default_reasoning_level": "medium",
        "supported_reasoning_levels": [
            {"effort": "medium", "description": "medium"}
        ],
        "shell_type": "shell_command",
        "visibility": "list",
        "supported_in_api": true,
        "priority": 1,
        "upgrade": null,
        "base_instructions": "base instructions",
        "model_messages": null,
        "supports_reasoning_summaries": false,
        "support_verbosity": false,
        "default_verbosity": null,
        "apply_patch_tool_type": null,
        "truncation_policy": {"mode": "bytes", "limit": 10000},
        "supports_parallel_tool_calls": false,
        "supports_image_detail_original": false,
        "context_window": 272000,
        "auto_compact_token_limit": null,
        "experimental_supported_tools": []
    }))
    .expect("deserialize test model info")
}

async fn request_input(prompt: &Prompt) -> Vec<ResponseItem> {
    let client = test_model_client();
    let provider = client
        .state
        .provider
        .api_provider()
        .await
        .expect("API provider");
    let thread_id = client.state.thread_id.to_string();
    let metadata = responses_metadata(
        "11111111-1111-4111-8111-111111111111",
        &thread_id,
        &thread_id,
        /*turn_id*/ None,
        format!("{thread_id}:0"),
        &client.state.session_source,
        /*parent_thread_id*/ None,
        TestCodexResponsesRequestKind::Turn,
    );

    client
        .build_responses_request(
            &provider,
            prompt,
            &test_model_info(),
            /*effort*/ None,
            ReasoningSummary::None,
            /*service_tier*/ None,
            &metadata,
        )
        .expect("responses request")
        .input
}

#[tokio::test]
async fn request_body_repairs_every_legacy_multi_agent_v2_function_name_without_mutating_prompt() {
    let prompt = Prompt {
        input: MULTI_AGENT_V2_FUNCTION_NAMES
            .iter()
            .map(|function_name| function_call(function_name, None))
            .collect(),
        tools: vec![multi_agent_v2_namespace(CUSTOM_NAMESPACE)],
        ..Default::default()
    };
    let original_input = prompt.input.clone();

    let request_input = request_input(&prompt).await;

    let expected = MULTI_AGENT_V2_FUNCTION_NAMES
        .iter()
        .map(|function_name| function_call(function_name, Some(CUSTOM_NAMESPACE)))
        .collect::<Vec<_>>();
    assert_eq!(request_input, expected);
    assert_eq!(prompt.input, original_input);
}

#[tokio::test]
async fn request_body_fails_closed_for_ambiguous_or_non_multi_agent_v2_tools() {
    let mut duplicate_family = MULTI_AGENT_V2_FUNCTION_NAMES.to_vec();
    duplicate_family.push("spawn_agent");
    let cases = [
        ("missing", Vec::new()),
        (
            "disabled_flat_family",
            MULTI_AGENT_V2_FUNCTION_NAMES
                .iter()
                .map(|name| flat_function(name))
                .collect(),
        ),
        (
            "partial_namespace",
            vec![namespace(CUSTOM_NAMESPACE, &["spawn_agent"])],
        ),
        (
            "v1_namespace",
            vec![namespace(
                "multi_agent_v1",
                &[
                    "close_agent",
                    "resume_agent",
                    "send_input",
                    "spawn_agent",
                    "wait_agent",
                ],
            )],
        ),
        (
            "flat_conflict",
            vec![
                multi_agent_v2_namespace(CUSTOM_NAMESPACE),
                flat_function("spawn_agent"),
            ],
        ),
        (
            "multiple_namespaces",
            vec![
                multi_agent_v2_namespace(CUSTOM_NAMESPACE),
                namespace("dynamic", &["spawn_agent"]),
            ],
        ),
        ("empty_namespace", vec![multi_agent_v2_namespace("")]),
        (
            "duplicate_family_child",
            vec![namespace(CUSTOM_NAMESPACE, &duplicate_family)],
        ),
    ];

    for (case, tools) in cases {
        let prompt = Prompt {
            input: vec![function_call("spawn_agent", None)],
            tools,
            ..Default::default()
        };

        let request_input = request_input(&prompt).await;

        assert_eq!(request_input, prompt.input, "case: {case}");
    }
}

#[tokio::test]
async fn request_body_preserves_explicit_namespaces_and_non_multi_agent_v2_calls() {
    let prompt = Prompt {
        input: vec![
            function_call("spawn_agent", Some("original_namespace")),
            function_call("unrelated_function", None),
            ResponseItem::Other,
        ],
        tools: vec![multi_agent_v2_namespace(CUSTOM_NAMESPACE)],
        ..Default::default()
    };

    let request_input = request_input(&prompt).await;

    assert_eq!(request_input, prompt.input);
}

#[tokio::test]
async fn request_body_combines_distinct_fragments_for_the_same_namespace() {
    let prompt = Prompt {
        input: vec![function_call("spawn_agent", None)],
        tools: vec![
            namespace(
                CUSTOM_NAMESPACE,
                &["spawn_agent", "send_message", "followup_task"],
            ),
            namespace(
                CUSTOM_NAMESPACE,
                &["wait_agent", "interrupt_agent", "list_agents"],
            ),
        ],
        ..Default::default()
    };

    let request_input = request_input(&prompt).await;

    assert_eq!(
        request_input,
        vec![function_call("spawn_agent", Some(CUSTOM_NAMESPACE))]
    );
}
