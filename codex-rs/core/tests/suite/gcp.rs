use anyhow::Context;
use anyhow::Result;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_model_provider_info::GCP_PROVIDER_ID;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceTool;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use test_case::test_case;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::client::ProviderAuthCommandFixture;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vertex_credential_file_is_retained_when_refreshing_after_401() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let auth_fixture = ProviderAuthCommandFixture::new(&["first-token", "second-token"])?;
    let auth = auth_fixture.auth();
    let credential_file = auth.cwd.join("adc.json").into_path_buf();
    let credential_log = auth.cwd.join("credential-env.txt");
    let inherited_credential_file = std::env::var_os("GOOGLE_APPLICATION_CREDENTIALS");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", "Bearer first-token"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .expect(1)
        .mount(&server)
        .await;
    let response = responses::mount_sse_once_match(
        &server,
        header("authorization", "Bearer second-token"),
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let mut provider = ModelProviderInfo::create_gcp_provider();
    provider.base_url = Some(format!("{}/v1", server.uri()));
    provider.auth = Some(auth);
    provider.credential_file = Some(credential_file.clone());
    let test = test_codex()
        .with_config(move |config| {
            config.model_provider_id = GCP_PROVIDER_ID.to_string();
            config.model_provider = provider;
        })
        .build_with_auto_env(&server)
        .await?;

    test.submit_turn("say hello").await?;

    assert_eq!(
        response.single_request().header("authorization").as_deref(),
        Some("Bearer second-token"),
    );
    let observed = std::fs::read_to_string(credential_log)?;
    let expected = credential_file.to_string_lossy();
    assert_eq!(
        observed.lines().collect::<Vec<_>>(),
        vec![expected.as_ref(); 2]
    );
    assert_eq!(
        std::env::var_os("GOOGLE_APPLICATION_CREDENTIALS"),
        inherited_credential_file,
    );
    Ok(())
}

#[test_case("gpt-6.1-sol"; "sol")]
#[test_case("gpt-6.1-terra"; "terra")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vertex_requests_use_command_auth_with_namespaces_and_tool_search(
    model: &str,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let auth_fixture = ProviderAuthCommandFixture::fixed_token("vertex-access-token")?;
    let mut provider = ModelProviderInfo::create_gcp_provider();
    provider.base_url = Some(format!(
        "{}/v1/projects/example-project/locations/global/publishers/openai/v1",
        server.uri()
    ));
    provider.auth = Some(auth_fixture.auth());

    let mut test = test_codex()
        .with_model_info_override(model, |model| {
            model.use_responses_lite = true;
            model.tool_mode = Some(ToolMode::CodeModeOnly);
            model.apply_patch_tool_type = Some(ApplyPatchToolType::Freeform);
            model.supports_search_tool = true;
            model.multi_agent_version = Some(MultiAgentVersion::V2);
        })
        .with_model(model)
        .with_config(move |config| {
            config.model_provider_id = GCP_PROVIDER_ID.to_string();
            config.model_provider = provider;
            config.base_instructions = Some("Vertex test instructions".to_string());
        })
        .build_with_auto_env(&server)
        .await?;
    let input_schema = json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false,
    });
    let dynamic_tool = DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
        name: "records".to_string(),
        description: "Record tools.".to_string(),
        tools: vec![
            DynamicToolNamespaceTool::Function(DynamicToolFunctionSpec {
                name: "status".to_string(),
                description: "Get record service status.".to_string(),
                input_schema: input_schema.clone(),
                defer_loading: false,
            }),
            DynamicToolNamespaceTool::Function(DynamicToolFunctionSpec {
                name: "lookup".to_string(),
                description: "Look up a record.".to_string(),
                input_schema: input_schema.clone(),
                defer_loading: true,
            }),
        ],
    });
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            dynamic_tools: vec![dynamic_tool],
            ..test.start_thread_options().await
        })
        .await?;
    test.codex = thread.thread;
    test.session_configured = thread.session_configured;

    test.submit_turn("Look up a record").await?;

    let request = response.single_request();
    assert_eq!(
        request.path(),
        "/v1/projects/example-project/locations/global/publishers/openai/v1/responses"
    );
    assert_eq!(
        request.header("authorization").as_deref(),
        Some("Bearer vertex-access-token")
    );
    assert_eq!(
        request.header("x-openai-internal-codex-responses-lite"),
        None
    );
    assert!(
        request.inputs_of_type("additional_tools").is_empty(),
        "Vertex request must omit Responses Lite additional_tools inputs"
    );
    let body = request.body_json();
    assert_eq!(body["model"], model);
    assert_eq!(request.instructions_text(), "Vertex test instructions");
    let tools = body["tools"]
        .as_array()
        .context("Vertex request should include model tools")?;
    assert_eq!(
        test.codex.multi_agent_version(),
        Some(MultiAgentVersion::V2)
    );
    assert!(tools.iter().any(|tool| tool["name"] == "collaboration"));
    let namespace = tools.iter().find(|tool| tool["name"] == "records");
    assert_eq!(
        namespace,
        Some(&json!({
            "type": "namespace",
            "name": "records",
            "description": "Record tools.",
            "tools": [{
                "type": "function",
                "name": "status",
                "description": "Get record service status.",
                "strict": false,
                "parameters": input_schema,
            }],
        }))
    );
    assert!(
        tools
            .iter()
            .any(|tool| tool["type"] == "tool_search" && tool["execution"] == "client"),
        "Vertex request should expose client-side tool search: {tools:?}"
    );
    assert!(
        tools
            .iter()
            .any(|tool| tool.get("name").and_then(Value::as_str) == Some("exec_command")),
        "Vertex request should preserve the exec_command function tool: {tools:?}"
    );

    Ok(())
}

#[test_case("gpt-6.1-sol"; "catalog model")]
#[test_case("vertex-test-model"; "fallback model")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vertex_requests_omit_reasoning_summaries(model: &str) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let auth_fixture = ProviderAuthCommandFixture::new(&["vertex-access-token"])?;
    let mut provider = ModelProviderInfo::create_gcp_provider();
    provider.base_url = Some(format!("{}/v1", server.uri()));
    provider.auth = Some(auth_fixture.auth());
    let test = test_codex()
        .with_model_info_override("gpt-6.1-sol", |model_info| {
            model_info.supports_reasoning_summary_parameter = true;
            model_info.default_reasoning_summary = ReasoningSummary::Detailed;
        })
        .with_model(model)
        .with_config(move |config| {
            config.model_provider_id = GCP_PROVIDER_ID.to_string();
            config.model_provider = provider;
            config.model_reasoning_effort = Some(ReasoningEffort::High);
            config.model_reasoning_summary = Some(ReasoningSummary::None);
            config
                .features
                .enable(Feature::ConcurrentReasoningSummaries)
                .expect("test config should allow feature update");
        })
        .build_with_auto_env(&server)
        .await?;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "say hello".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                summary: Some(ReasoningSummary::Concise),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let body = response.single_request().body_json();
    assert_eq!(body["model"], model);
    assert_eq!(body["reasoning"], json!({"effort": "high"}));
    assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
    assert_eq!(body.get("stream_options"), None);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_vertex_credentials_explain_how_to_reauthenticate() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("unused-response"),
            responses::ev_completed("unused-response"),
        ]),
    )
    .await;
    let auth_fixture = ProviderAuthCommandFixture::new(&[])?;
    let mut provider = ModelProviderInfo::create_gcp_provider();
    provider.base_url = Some(format!(
        "{}/v1/projects/example-project/locations/global/publishers/openai/v1",
        server.uri()
    ));
    provider.auth = Some(auth_fixture.auth());

    let test = test_codex()
        .with_model("vertex-test-model")
        .with_config(move |config| {
            config.model_provider_id = GCP_PROVIDER_ID.to_string();
            config.model_provider = provider;
        })
        .build_with_auto_env(&server)
        .await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "say hello".into(),
            text_elements: Vec::new(),
        }]))
        .await?;

    let EventMsg::Error(error) =
        wait_for_event(&test.codex, |event| matches!(event, EventMsg::Error(_))).await
    else {
        unreachable!();
    };
    assert_eq!(
        error.message,
        "Google Cloud credentials are unavailable or expired. Run \
`gcloud auth application-default login` to reauthenticate."
    );
    assert!(response.requests().is_empty());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vertex_guardian_requests_use_luna() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("parent-tool"),
                responses::ev_function_call(
                    "reviewed-tool",
                    "exec_command",
                    r#"{"cmd":"true","sandbox_permissions":"require_escalated","justification":"test"}"#,
                ),
                responses::ev_completed("parent-tool"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("guardian-review"),
                responses::ev_assistant_message("guardian", r#"{"outcome":"deny"}"#),
                responses::ev_completed("guardian-review"),
            ]),
            responses::sse(vec![responses::ev_completed("parent-complete")]),
        ],
    )
    .await;
    let auth_fixture = ProviderAuthCommandFixture::fixed_token("vertex-access-token")?;
    let mut provider = ModelProviderInfo::create_gcp_provider();
    provider.base_url = Some(format!("{}/v1", server.uri()));
    provider.auth = Some(auth_fixture.auth());
    let test = test_codex()
        .with_model("gpt-6.1-sol")
        .with_config(move |config| {
            config.model_provider_id = GCP_PROVIDER_ID.to_string();
            config.model_provider = provider;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        })
        .build_with_auto_env(&server)
        .await?;

    let models = test.thread_manager.get_models_manager().try_list_models()?;
    insta::assert_debug_snapshot!(
        "gcp_model_picker",
        models
            .iter()
            .map(|model| (&model.model, model.show_in_picker, model.is_default))
            .collect::<Vec<_>>()
    );

    test.submit_text_turn("run a command").await?;

    let requests = response.requests();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.body_json()["model"].clone())
            .collect::<Vec<_>>(),
        vec![
            json!("gpt-6.1-sol"),
            json!("gpt-6-luna"),
            json!("gpt-6.1-sol")
        ]
    );
    let guardian_request = &requests[1];
    assert_eq!(guardian_request.path(), "/v1/responses");
    assert_eq!(
        guardian_request.header("authorization").as_deref(),
        Some("Bearer vertex-access-token")
    );

    Ok(())
}
