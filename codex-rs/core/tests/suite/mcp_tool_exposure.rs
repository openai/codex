use anyhow::Result;
use codex_config::Constrained;
use codex_core::config::Config;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_features::Feature;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::McpResourceClient;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_CREATE_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::apps_enabled_builder;
use core_test_support::apps_test_server::search_capable_apps_builder;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::namespace_child_tool;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::wait_for_event;
use core_test_support::wait_for_mcp_server;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

struct McpResourceClientCapture {
    client: Arc<Mutex<Option<McpResourceClient>>>,
}

impl ThreadLifecycleContributor<Config> for McpResourceClientCapture {
    fn on_thread_start<'a>(
        &'a self,
        input: ThreadStartInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let client = input
                .mcp_resource_client
                .as_ref()
                .expect("host should supply an MCP resource client");
            *self
                .client
                .lock()
                .expect("capture lock should not be poisoned") = Some(client.as_ref().clone());
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn out_of_band_resource_read_reconciles_the_published_mcp_runtime() -> Result<()> {
    let server = responses::start_mock_server().await;

    let captured_client = Arc::new(Mutex::new(None));
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.thread_lifecycle_contributor(Arc::new(McpResourceClientCapture {
        client: Arc::clone(&captured_client),
    }));
    let test = core_test_support::test_codex::test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .build(&server)
        .await?;
    let resource_client = captured_client
        .lock()
        .expect("capture lock should not be poisoned")
        .clone()
        .expect("thread start should capture the MCP resource client");
    assert!(!resource_client.has_server("refreshed").await);

    let mut refresh_config = test.config.clone();
    let user_config_path = refresh_config.codex_home.join("config.toml");
    let user_config: toml::Value = toml::from_str(&format!(
        r#"
[mcp_servers.refreshed]
url = "{}/mcp"
startup_timeout_sec = 0.1
"#,
        server.uri()
    ))?;
    let refreshed_servers = user_config
        .get("mcp_servers")
        .cloned()
        .map(HashMap::<String, codex_config::types::McpServerConfig>::deserialize)
        .transpose()?
        .expect("test config should define MCP servers");
    refresh_config
        .mcp_servers
        .set(refreshed_servers)
        .expect("test config should allow MCP servers");
    refresh_config.config_layer_stack = refresh_config
        .config_layer_stack
        .with_user_config(&user_config_path, user_config)?;
    test.codex.refresh_runtime_config(refresh_config).await;
    test.codex.submit(Op::RefreshMcpServers).await?;

    let _ = test
        .codex
        .read_mcp_resource("refreshed", "test://resource")
        .await;
    assert!(resource_client.has_server("refreshed").await);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_only_exposes_direct_model_only_mcp_namespaces() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let apps_server = AppsTestServer::mount_searchable(&server).await?;
    let response = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = search_capable_apps_builder(apps_server.chatgpt_base_url.clone())
        .with_config(move |config| {
            config
                .features
                .enable(Feature::CodeModeOnly)
                .expect("test config should allow feature update");
            config.code_mode.direct_only_tool_namespaces =
                vec![SEARCH_CALENDAR_NAMESPACE.to_string()];
        });
    let test = builder.build(&server).await?;
    test.submit_turn("inspect directly exposed MCP tools")
        .await?;
    let body = response.single_request().body_json();
    let tools = body
        .get("tools")
        .and_then(Value::as_array)
        .expect("request should contain tools");

    assert!(
        namespace_child_tool(
            &body,
            SEARCH_CALENDAR_NAMESPACE,
            SEARCH_CALENDAR_CREATE_TOOL,
        )
        .is_some(),
        "configured MCP namespace should remain top-level: {body}"
    );
    assert!(
        !tools.iter().any(|tool| {
            tool.get("name")
                .or_else(|| tool.get("type"))
                .and_then(Value::as_str)
                == Some("tool_search")
        }),
        "configured MCP namespace should not be deferred: {body}"
    );
    let exec_description = tools.iter().find_map(|tool| {
        (tool.get("name").and_then(Value::as_str) == Some("exec"))
            .then(|| tool.get("description").and_then(Value::as_str))
            .flatten()
    });
    assert!(
        exec_description.is_some_and(|description| {
            !description.contains("mcp__codex_apps__calendar_create_event(args:")
        }),
        "direct-model-only MCP namespace should not be available through exec: {body}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apps_guidance_appears_after_recovery_between_sampling_requests() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (apps_server, startup_control) =
        AppsTestServer::mount_with_startup_control(&server).await?;
    // The exact step is captured once before the first request; its background retry can recover.
    startup_control.fail_next_initialize_attempts(/*attempts*/ 1);
    let call_id = "pause-for-apps";
    let response = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    call_id,
                    "request_user_input",
                    &json!({
                        "questions": [{
                            "id": "continue",
                            "header": "Continue",
                            "question": "Continue after Apps recovers?",
                            "options": [{
                                "label": "Yes (Recommended)",
                                "description": "Continue the test."
                            }, {
                                "label": "No",
                                "description": "Stop the test."
                            }]
                        }]
                    })
                    .to_string(),
                ),
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
    let mut builder =
        apps_enabled_builder(apps_server.chatgpt_base_url.clone()).with_config(|config| {
            config
                .features
                .enable(Feature::DefaultModeRequestUserInput)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "use an app after it recovers".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    let request = tokio::time::timeout(Duration::from_secs(3), async {
        let mut request = None;
        let mut apps_ready = false;
        while request.is_none() || !apps_ready {
            let event = test
                .codex
                .next_event()
                .await
                .expect("event stream should stay open");
            match event.msg {
                EventMsg::RequestUserInput(next_request) => request = Some(next_request),
                EventMsg::McpStartupUpdate(update)
                    if update.server == CODEX_APPS_MCP_SERVER_NAME
                        && matches!(
                            update.status,
                            codex_protocol::protocol::McpStartupStatus::Ready
                        ) =>
                {
                    apps_ready = true;
                }
                _ => {}
            }
        }
        request.expect("request user input event")
    })
    .await
    .expect("Apps should recover before the second sampling request");

    let initial_requests = response.requests();
    assert_eq!(initial_requests.len(), 1);
    let initial_request = &initial_requests[0];
    assert_eq!(
        initial_request
            .message_input_texts("developer")
            .iter()
            .filter(|text| text.contains("<apps_instructions>"))
            .count(),
        0
    );

    test.codex
        .submit(Op::UserInputAnswer {
            id: request.turn_id,
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    "continue".to_string(),
                    RequestUserInputAnswer {
                        answers: vec!["Yes (Recommended)".to_string()],
                    },
                )]),
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = response.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1]
            .message_input_texts("developer")
            .iter()
            .filter(|text| text.contains("<apps_instructions>"))
            .count(),
        1
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn later_follow_up_uses_background_recovered_apps_after_mid_thread_startup_failures()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (apps_server, startup_control) =
        AppsTestServer::mount_with_startup_control(&server).await?;
    let response = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "initial turn"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "recovery-trigger turn"),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-3", "recovered follow-up turn"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    let mut builder = search_capable_apps_builder(apps_server.chatgpt_base_url.clone())
        .with_config(move |config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::Never);
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .expect("test config should allow disabled permissions");
            config
                .features
                .enable(Feature::CodeModeOnly)
                .expect("test config should allow feature update");
            config.code_mode.direct_only_tool_namespaces =
                vec![SEARCH_CALENDAR_NAMESPACE.to_string()];
        });
    let test = builder.build(&server).await?;
    wait_for_mcp_server(&test.codex, CODEX_APPS_MCP_SERVER_NAME).await?;
    test.submit_turn("use Calendar before refreshing MCP")
        .await?;

    let initial_request = response.requests()[0].body_json();
    assert!(
        namespace_child_tool(
            &initial_request,
            SEARCH_CALENDAR_NAMESPACE,
            SEARCH_CALENDAR_CREATE_TOOL,
        )
        .is_some(),
        "Calendar should be available before the MCP refresh: {initial_request}"
    );

    tokio::fs::remove_dir_all(test.codex_home_path().join("cache/codex_apps_tools")).await?;
    startup_control.fail_next_initialize_attempts(/*attempts*/ 1);
    test.codex
        .set_openai_form_elicitation_support(/*supported*/ true)
        .await?;
    test.codex.submit(Op::RefreshMcpServers).await?;
    test.submit_turn("use Calendar after transient Apps startup failures")
        .await?;
    tokio::time::timeout(Duration::from_secs(5), async {
        while startup_control.initialize_attempts() < 3 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("background Apps reconnect should complete");
    test.submit_turn("use Calendar after background Apps recovery")
        .await?;

    let requests = response.requests();
    assert_eq!(requests.len(), 3);
    let recovered_request = requests[2].body_json();
    assert!(
        namespace_child_tool(
            &recovered_request,
            SEARCH_CALENDAR_NAMESPACE,
            SEARCH_CALENDAR_CREATE_TOOL,
        )
        .is_some(),
        "Calendar should recover on the follow-up turn: {recovered_request}",
    );
    assert_eq!(startup_control.initialize_attempts(), 3);

    Ok(())
}
