use anyhow::Result;
use codex_core::config::Config;
use codex_features::Feature;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::MultiAgentVersion;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;

fn catalog_model(slug: &str) -> ModelInfo {
    ModelInfo {
        visibility: ModelVisibility::List,
        used_fallback_model_metadata: false,
        supports_search_tool: false,
        ..model_info_from_slug(slug)
    }
}

fn tool_names(body: &Value) -> Vec<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| {
                    tool.get("name")
                        .or_else(|| tool.get("type"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn namespace_child_tool_names(body: &Value, namespace: &str) -> Vec<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .and_then(|tools| {
            tools.iter().find_map(|tool| {
                if tool.get("type").and_then(Value::as_str) == Some("namespace")
                    && tool.get("name").and_then(Value::as_str) == Some(namespace)
                {
                    tool.get("tools").and_then(Value::as_array).map(|children| {
                        children
                            .iter()
                            .filter_map(|child| {
                                child
                                    .get("name")
                                    .and_then(Value::as_str)
                                    .map(str::to_string)
                            })
                            .collect()
                    })
                } else {
                    None
                }
            })
        })
        .unwrap_or_default()
}

fn selected_tool_names(body: &Value, selected: &[&str]) -> Vec<String> {
    tool_names(body)
        .into_iter()
        .filter(|name| selected.contains(&name.as_str()))
        .collect()
}

fn tool_description<'a>(body: &'a Value, name: &str) -> Option<&'a str> {
    body.get("tools")
        .and_then(Value::as_array)
        .and_then(|tools| {
            tools
                .iter()
                .find(|tool| tool.get("name").and_then(Value::as_str) == Some(name))
        })
        .and_then(|tool| tool.get("description"))
        .and_then(Value::as_str)
}

async fn response_body_for_catalog_model(
    catalog_model: ModelInfo,
    configure: impl FnOnce(&mut Config) + Send + 'static,
) -> Result<Value> {
    let server = responses::start_mock_server().await;
    let model_slug = catalog_model.slug.clone();
    let response_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_config(move |config| {
        config.model = Some(model_slug);
        config.model_catalog = Some(ModelsResponse {
            models: vec![catalog_model],
        });
        configure(config);
    });
    let test = builder.build(&server).await?;
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "list tools".into(),
                text_elements: Vec::new(),
            }],
            environments: None,
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    Ok(response_mock.single_request().body_json())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_tool_mode_selector_overrides_feature_flags() -> Result<()> {
    let mut direct_model = catalog_model("test-tool-mode-direct");
    direct_model.tool_mode = Some(ToolMode::Direct);
    let direct_body = response_body_for_catalog_model(direct_model, |config| {
        config
            .features
            .enable(Feature::CodeModeOnly)
            .expect("test config should allow feature update");
    })
    .await?;
    let direct_tools = tool_names(&direct_body);
    assert!(
        direct_tools
            .iter()
            .all(|name| name != codex_code_mode::PUBLIC_TOOL_NAME
                && name != codex_code_mode::WAIT_TOOL_NAME),
        "direct mode should override enabled code mode flags: {direct_tools:?}"
    );

    let mut code_mode_only_model = catalog_model("test-tool-mode-code-mode-only");
    code_mode_only_model.tool_mode = Some(ToolMode::CodeModeOnly);
    let code_mode_only_body = response_body_for_catalog_model(code_mode_only_model, |_| {}).await?;
    assert_eq!(
        tool_names(&code_mode_only_body),
        vec![
            codex_code_mode::PUBLIC_TOOL_NAME.to_string(),
            codex_code_mode::WAIT_TOOL_NAME.to_string(),
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_multi_agent_version_selector_overrides_feature_flags() -> Result<()> {
    let mut v1_model = catalog_model("test-multi-agent-v1");
    v1_model.multi_agent_version = Some(MultiAgentVersion::V1);
    let v1_body = response_body_for_catalog_model(v1_model, |config| {
        config
            .features
            .enable(Feature::MultiAgentV2)
            .expect("test config should allow feature update");
        config.multi_agent_v2.root_agent_usage_hint_text =
            Some("V2 guidance must not reach v1 models.".to_string());
    })
    .await?;
    assert_eq!(
        namespace_child_tool_names(&v1_body, "multi_agent_v1"),
        vec![
            "close_agent".to_string(),
            "resume_agent".to_string(),
            "send_input".to_string(),
            "spawn_agent".to_string(),
            "wait_agent".to_string(),
        ]
    );
    assert_eq!(
        selected_tool_names(&v1_body, &["send_message", "followup_task", "list_agents"]),
        Vec::<String>::new()
    );
    assert!(
        !v1_body
            .to_string()
            .contains("V2 guidance must not reach v1 models."),
        "v1 models should not receive v2 usage hints: {v1_body:?}"
    );

    let mut v2_model = catalog_model("test-multi-agent-v2");
    v2_model.multi_agent_version = Some(MultiAgentVersion::V2);
    let v2_body = response_body_for_catalog_model(v2_model, |config| {
        config
            .features
            .disable(Feature::Collab)
            .expect("test config should allow feature update");
        config
            .features
            .disable(Feature::MultiAgentV2)
            .expect("test config should allow feature update");
        config.multi_agent_v2.max_concurrent_threads_per_session = 17;
        config.multi_agent_v2.root_agent_usage_hint_text =
            Some("V2 guidance should reach v2 models.".to_string());
    })
    .await?;
    assert_eq!(
        selected_tool_names(
            &v2_body,
            &[
                "spawn_agent",
                "send_input",
                "resume_agent",
                "wait_agent",
                "close_agent",
                "send_message",
                "followup_task",
                "list_agents",
            ],
        ),
        vec![
            "spawn_agent".to_string(),
            "send_message".to_string(),
            "followup_task".to_string(),
            "wait_agent".to_string(),
            "close_agent".to_string(),
            "list_agents".to_string(),
        ]
    );
    assert_eq!(
        namespace_child_tool_names(&v2_body, "multi_agent_v1"),
        Vec::<String>::new()
    );
    assert!(
        tool_description(&v2_body, "spawn_agent").is_some_and(
            |description| description.contains("max_concurrent_threads_per_session = 17")
        ),
        "v2 spawn_agent should advertise the configured concurrency cap: {v2_body:?}"
    );
    assert!(
        v2_body
            .to_string()
            .contains("V2 guidance should reach v2 models."),
        "v2 models should receive v2 usage hints: {v2_body:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_omitted_and_unknown_multi_agent_versions_follow_feature_flags() -> Result<()> {
    let omitted_body =
        response_body_for_catalog_model(catalog_model("test-multi-agent-omitted"), |config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow feature update");
            config
                .features
                .disable(Feature::MultiAgentV2)
                .expect("test config should allow feature update");
        })
        .await?;

    let mut unknown_model =
        serde_json::to_value(catalog_model("test-multi-agent-unknown-version"))?;
    unknown_model["multi_agent_version"] = Value::String("future_multi_agent_version".to_string());
    let unknown_model = serde_json::from_value::<ModelInfo>(unknown_model)?;
    let unknown_body = response_body_for_catalog_model(unknown_model, |config| {
        config
            .features
            .enable(Feature::Collab)
            .expect("test config should allow feature update");
        config
            .features
            .disable(Feature::MultiAgentV2)
            .expect("test config should allow feature update");
    })
    .await?;

    assert_eq!(
        (
            namespace_child_tool_names(&omitted_body, "multi_agent_v1"),
            namespace_child_tool_names(&unknown_body, "multi_agent_v1"),
        ),
        (
            vec![
                "close_agent".to_string(),
                "resume_agent".to_string(),
                "send_input".to_string(),
                "spawn_agent".to_string(),
                "wait_agent".to_string(),
            ],
            vec![
                "close_agent".to_string(),
                "resume_agent".to_string(),
                "send_input".to_string(),
                "spawn_agent".to_string(),
                "wait_agent".to_string(),
            ],
        )
    );

    Ok(())
}
