use anyhow::Result;
use codex_core::config::Config;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::manager::SharedModelsManager;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_models_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;

const CHILD_MODEL: &str = "test-multi-agent-child";
const ROOT_MODEL: &str = "test-multi-agent-root";
const ROOT_PROMPT: &str = "spawn a child";
const UNSUPPORTED_CODE_MODE_WARNING: &str = "does not advertise Code Mode support";

struct RemoteModelResponse {
    body: Value,
    warnings: Vec<String>,
}

fn remote_model(slug: &str) -> ModelInfo {
    ModelInfo {
        visibility: ModelVisibility::List,
        used_fallback_model_metadata: false,
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

async fn wait_for_model_available(manager: &SharedModelsManager, slug: &str) -> ModelPreset {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(model) = manager
            .list_models(RefreshStrategy::Online)
            .await
            .iter()
            .find(|model| model.model == slug)
            .cloned()
        {
            return model;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for the remote model {slug} to appear");
        }
        sleep(Duration::from_millis(25)).await;
    }
}

async fn response_for_remote_model(
    remote_model: ModelInfo,
    configure: impl FnOnce(&mut Config) + Send + 'static,
) -> Result<RemoteModelResponse> {
    let server = responses::start_mock_server().await;
    let model_slug = remote_model.slug.clone();
    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model],
        },
    )
    .await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(configure);
    let test = builder.build(&server).await?;
    let models_manager = test.thread_manager.get_models_manager();
    let available_model = wait_for_model_available(&models_manager, &model_slug).await;
    assert_eq!(available_model.model, model_slug);
    assert_eq!(models_mock.requests().len(), 1);

    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some(model_slug),
            ..Default::default()
        },
    )
    .await?;
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "list tools".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    let mut warnings = Vec::new();
    loop {
        match wait_for_event(&test.codex, |_| true).await {
            EventMsg::Warning(warning) => warnings.push(warning.message),
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    Ok(RemoteModelResponse {
        body: response_mock.single_request().body_json(),
        warnings,
    })
}

async fn response_body_for_remote_model(
    remote_model: ModelInfo,
    configure: impl FnOnce(&mut Config) + Send + 'static,
) -> Result<Value> {
    Ok(response_for_remote_model(remote_model, configure)
        .await?
        .body)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_tool_mode_selector_overrides_feature_flags() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let mut direct_model = remote_model("test-tool-mode-direct");
    direct_model.tool_mode = Some(ToolMode::Direct);
    let direct_body = response_body_for_remote_model(direct_model, |config| {
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

    let mut code_mode_only_model = remote_model("test-tool-mode-code-mode-only");
    code_mode_only_model.tool_mode = Some(ToolMode::CodeModeOnly);
    code_mode_only_model.input_modalities = vec![InputModality::Text, InputModality::Image];
    let code_mode_only_body = response_body_for_remote_model(code_mode_only_model, |_| {}).await?;
    assert_eq!(
        tool_names(&code_mode_only_body),
        vec![
            // Code-mode entrypoints.
            codex_code_mode::PUBLIC_TOOL_NAME.to_string(),
            codex_code_mode::WAIT_TOOL_NAME.to_string(),
            "request_user_input".to_string(),
            // Hosted Responses tools.
            "web_search".to_string(),
            "image_generation".to_string(),
        ]
    );

    let unsupported_model = remote_model("test-tool-mode-unsupported");
    let unsupported_response = response_for_remote_model(unsupported_model, |config| {
        config
            .features
            .enable(Feature::CodeModeOnly)
            .expect("test config should allow feature update");
    })
    .await?;
    assert!(
        tool_names(&unsupported_response.body)
            .iter()
            .any(|name| name == codex_code_mode::PUBLIC_TOOL_NAME)
    );
    assert_eq!(
        unsupported_response
            .warnings
            .iter()
            .filter(|warning| warning.contains(UNSUPPORTED_CODE_MODE_WARNING))
            .count(),
        1
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unsupported_code_mode_warning_is_emitted_each_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let model_slug = "test-tool-mode-warning-each-turn";
    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model(model_slug)],
        },
    )
    .await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let test = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodeMode)
                .expect("test config should allow feature update");
        })
        .build(&server)
        .await?;
    let models_manager = test.thread_manager.get_models_manager();
    let available_model = wait_for_model_available(&models_manager, model_slug).await;
    assert_eq!(available_model.model, model_slug);
    assert_eq!(models_mock.requests().len(), 1);

    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some(model_slug.to_string()),
            ..Default::default()
        },
    )
    .await?;

    let mut warning_counts = Vec::new();
    for prompt in ["first turn", "second turn"] {
        test.codex
            .submit(Op::UserInput {
                items: vec![UserInput::Text {
                    text: prompt.to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
                responsesapi_client_metadata: None,
                additional_context: Default::default(),
                thread_settings: Default::default(),
            })
            .await?;

        let mut warning_count = 0;
        loop {
            match wait_for_event(&test.codex, |_| true).await {
                EventMsg::Warning(warning)
                    if warning.message.contains(UNSUPPORTED_CODE_MODE_WARNING) =>
                {
                    warning_count += 1;
                }
                EventMsg::TurnComplete(_) => break,
                _ => {}
            }
        }
        warning_counts.push(warning_count);
    }

    assert_eq!(warning_counts, vec![1, 1]);
    assert_eq!(response_mock.requests().len(), 2);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_multi_agent_selector_overrides_feature_flags() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let mut v2_model = remote_model("test-multi-agent-v2");
    v2_model.multi_agent_version = Some(MultiAgentVersion::V2);
    let v2_body = response_body_for_remote_model(v2_model, |config| {
        config.agent_max_threads = Some(3);
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
    assert!(tool_names(&v2_body).contains(&"send_message".to_string()));

    let mut disabled_model = remote_model("test-multi-agent-disabled");
    disabled_model.multi_agent_version = Some(MultiAgentVersion::Disabled);
    let disabled_body = response_body_for_remote_model(disabled_model, |config| {
        config
            .features
            .enable(Feature::MultiAgentV2)
            .expect("test config should allow feature update");
    })
    .await?;
    let disabled_tools = tool_names(&disabled_body);
    assert!(disabled_tools.iter().all(|name| !matches!(
        name.as_str(),
        "multi_agent_v1" | "spawn_agent" | "send_message" | "wait_agent" | "list_agents"
    )));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_multi_agent_selector_uses_model_selected_before_first_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = wiremock::MockServer::start().await;
    let mut initial_model = remote_model(ROOT_MODEL);
    initial_model.multi_agent_version = Some(MultiAgentVersion::V1);
    let mut selected_model = remote_model(CHILD_MODEL);
    selected_model.multi_agent_version = Some(MultiAgentVersion::V2);
    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![initial_model, selected_model],
        },
    )
    .await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config.model = Some(ROOT_MODEL.to_string());
        });
    let test = builder.build(&server).await?;
    assert_eq!(
        (
            models_mock.requests().len(),
            test.codex.multi_agent_version(),
        ),
        (1, None)
    );

    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some(CHILD_MODEL.to_string()),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(test.codex.multi_agent_version(), None);

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: ROOT_PROMPT.into(),
                text_elements: Vec::new(),
            }],
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

    assert_eq!(
        (
            models_mock.requests().len(),
            test.codex.multi_agent_version(),
            tool_names(
                &response_mock
                    .last_request()
                    .expect("expected response request")
                    .body_json(),
            )
            .contains(&"send_message".to_string()),
        ),
        (1, Some(MultiAgentVersion::V2), true)
    );

    Ok(())
}
