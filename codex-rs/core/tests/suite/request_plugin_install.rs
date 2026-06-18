#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used)]

use anyhow::Result;
use codex_config::types::ToolSuggestDisabledTool;
use codex_config::types::ToolSuggestDiscoverable;
use codex_config::types::ToolSuggestDiscoverableType;
use codex_core::config::Config;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
use codex_protocol::approvals::ElicitationAction;
use codex_protocol::approvals::ElicitationRequest;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use serde_json::Value;
use serde_json::json;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

const TOOL_SEARCH_TOOL_NAME: &str = "tool_search";
const LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME: &str = "list_available_plugins_to_install";
const REQUEST_PLUGIN_INSTALL_TOOL_NAME: &str = "request_plugin_install";
const DISCOVERABLE_GMAIL_ID: &str = "connector_68df038e0ba48191908c8434991bbac2";

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

fn configure_apps_without_search_tool(config: &mut Config, apps_base_url: &str) {
    for feature in [
        Feature::Apps,
        Feature::Plugins,
        Feature::RemotePlugin,
        Feature::ToolSuggest,
    ] {
        config
            .features
            .enable(feature)
            .expect("test config should allow feature update");
    }
    let mut model_catalog = bundled_models_response()
        .unwrap_or_else(|err| panic!("bundled models.json should parse: {err}"));
    let model = model_catalog
        .models
        .iter_mut()
        .find(|model| model.slug == "gpt-5.4")
        .expect("gpt-5.4 exists in bundled models.json");
    config.chatgpt_base_url = apps_base_url.to_string();
    config.model = Some("gpt-5.4".to_string());
    config.tool_suggest.discoverables = vec![ToolSuggestDiscoverable {
        kind: ToolSuggestDiscoverableType::Connector,
        id: DISCOVERABLE_GMAIL_ID.to_string(),
    }];
    model.supports_search_tool = false;
    config.model_catalog = Some(model_catalog);
}

async fn mount_recommendations(server: &wiremock::MockServer, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path("/ps/plugins/suggested"))
        .and(query_param("scope", "GLOBAL"))
        .respond_with(response)
        .mount(server)
        .await;
}

fn assert_legacy_tools(body: &Value) {
    let tools = tool_names(body);
    assert!(!tools.iter().any(|name| name == TOOL_SEARCH_TOOL_NAME));
    assert!(
        tools
            .iter()
            .any(|name| name == LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME),
        "legacy mode should expose {LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME}: {tools:?}"
    );
    assert!(
        tools
            .iter()
            .any(|name| name == REQUEST_PLUGIN_INSTALL_TOOL_NAME),
        "legacy mode should expose {REQUEST_PLUGIN_INSTALL_TOOL_NAME}: {tools:?}"
    );
}

async fn build_test(
    server: &wiremock::MockServer,
    apps_server: &AppsTestServer,
) -> Result<TestCodex> {
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config({
            let apps_base_url = apps_server.chatgpt_base_url.clone();
            move |config| configure_apps_without_search_tool(config, apps_base_url.as_str())
        });
    builder.build(server).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_false_preserves_legacy_workflow() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    mount_recommendations(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({"enabled": false, "plugins": []})),
    )
    .await;
    let call_id = "list-installable-tools";
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME, "{}"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let test = build_test(&server, &apps_server).await?;
    test.submit_turn_with_approval_and_permission_profile(
        "list tools",
        AskForApproval::Never,
        PermissionProfile::Disabled,
    )
    .await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    let request = &requests[0];
    assert!(
        !request
            .message_input_texts("user")
            .join("\n")
            .contains("<recommended_plugins>")
    );
    assert_legacy_tools(&request.body_json());
    let output = requests[1]
        .function_call_output_text(call_id)
        .expect("list tool output");
    let output: Value = serde_json::from_str(&output)?;
    assert!(output["tools"].as_array().is_some_and(|tools| {
        tools
            .iter()
            .any(|tool| tool["id"] == DISCOVERABLE_GMAIL_ID && tool["tool_type"] == "connector")
    }));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn endpoint_mode_injects_candidates_hides_list_and_rejects_invented_ids() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    mount_recommendations(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({
            "enabled": true,
            "plugins": [
                {
                    "id": "plugin_google_calendar",
                    "name": "google-calendar",
                    "status": "ENABLED",
                    "installation_policy": "AVAILABLE",
                    "release": {"display_name": "Google Calendar"}
                },
                {
                    "id": "plugin_github",
                    "name": "github",
                    "status": "ENABLED",
                    "installation_policy": "AVAILABLE",
                    "release": {"display_name": "GitHub"}
                }
            ]
        })),
    )
    .await;
    let call_id = "invented-plugin";
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    call_id,
                    REQUEST_PLUGIN_INSTALL_TOOL_NAME,
                    &serde_json::to_string(&json!({
                        "plugin_id": "invented@openai-curated-remote",
                        "suggest_reason": "Try this"
                    }))?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let test = build_test(&server, &apps_server).await?;

    test.submit_turn("suggest a plugin").await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    let contextual_user_message = requests[0].message_input_texts("user").join("\n");
    assert!(contextual_user_message.contains("<recommended_plugins>"));
    assert!(contextual_user_message.contains("github@openai-curated-remote"));
    assert!(contextual_user_message.contains("google-calendar@openai-curated-remote"));
    let body = requests[0].body_json();
    let tools = tool_names(&body);
    assert!(
        !tools
            .iter()
            .any(|name| name == LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME)
    );
    assert!(
        tools
            .iter()
            .any(|name| name == REQUEST_PLUGIN_INSTALL_TOOL_NAME)
    );
    let output = requests[1]
        .function_call_output_text(call_id)
        .expect("request tool output");
    assert!(output.contains("<recommended_plugins> list"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn endpoint_recommendation_adds_install_identity_only_to_elicitation_metadata() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    const REMOTE_PLUGIN_ID: &str = "plugin_connector_github";
    const APP_CONNECTOR_ID: &str = "connector_github";

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    mount_recommendations(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({
            "enabled": true,
            "plugins": [{
                "id": REMOTE_PLUGIN_ID,
                "name": "github",
                "status": "ENABLED",
                "installation_policy": "AVAILABLE",
                "release": {
                    "display_name": "GitHub",
                    "app_ids": [APP_CONNECTOR_ID]
                }
            }]
        })),
    )
    .await;
    let call_id = "install-github";
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    call_id,
                    REQUEST_PLUGIN_INSTALL_TOOL_NAME,
                    &serde_json::to_string(&json!({
                        "plugin_id": "github@openai-curated-remote",
                        "suggest_reason": "Use GitHub for this request"
                    }))?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let test = build_test(&server, &apps_server).await?;
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.config.cwd.as_path());

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "use GitHub".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: ThreadSettingsOverrides {
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: test.session_configured.model.clone(),
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    let elicitation = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::ElicitationRequest(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    let ElicitationRequest::Form {
        meta: Some(meta), ..
    } = &elicitation.request
    else {
        panic!("expected form elicitation metadata");
    };
    assert_eq!(meta["remote_plugin_id"], REMOTE_PLUGIN_ID);
    assert_eq!(meta["app_connector_ids"], json!([APP_CONNECTOR_ID]));

    test.codex
        .submit(Op::ResolveElicitation {
            server_name: elicitation.server_name,
            request_id: elicitation.id,
            decision: ElicitationAction::Decline,
            content: None,
            meta: None,
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    for request in requests {
        let body = request.body_json().to_string();
        assert!(!body.contains(REMOTE_PLUGIN_ID));
        assert!(!body.contains(APP_CONNECTOR_ID));
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn endpoint_mode_with_no_eligible_candidates_exposes_no_suggestion_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    mount_recommendations(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({
            "enabled": true,
            "plugins": [{
                "id": "plugin_google_calendar",
                "name": "google-calendar",
                "release": {"display_name": "Google Calendar"}
            }]
        })),
    )
    .await;
    let mock = mount_sse_once(
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
        .with_config({
            let apps_base_url = apps_server.chatgpt_base_url.clone();
            move |config| {
                configure_apps_without_search_tool(config, apps_base_url.as_str());
                config.tool_suggest.disabled_tools = vec![ToolSuggestDisabledTool::plugin(
                    "google-calendar@openai-curated-remote",
                )];
            }
        });
    let test = builder.build(&server).await?;

    test.submit_turn("list tools").await?;

    let request = mock.single_request();
    assert!(
        !request
            .message_input_texts("user")
            .join("\n")
            .contains("<recommended_plugins>")
    );
    let tools = tool_names(&request.body_json());
    assert!(
        !tools
            .iter()
            .any(|name| name == LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME)
    );
    assert!(
        !tools
            .iter()
            .any(|name| name == REQUEST_PLUGIN_INSTALL_TOOL_NAME)
    );
    Ok(())
}
