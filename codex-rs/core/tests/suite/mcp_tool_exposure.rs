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
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_CREATE_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::search_capable_apps_builder;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::ContextSnapshotRenderMode;
use core_test_support::responses;
use core_test_support::responses::ResponsesRequest;
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
use pretty_assertions::assert_eq;
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

fn format_labeled_requests_snapshot(
    scenario: &str,
    sections: &[(&str, &ResponsesRequest)],
) -> String {
    context_snapshot::format_labeled_requests_snapshot(
        scenario,
        sections,
        &ContextSnapshotOptions::default()
            .strip_capability_instructions()
            .render_mode(ContextSnapshotRenderMode::KindWithTextPrefix { max_chars: 96 }),
    )
}

fn enable_deferred_tool_world_state_without_agents(config: &mut Config) {
    config.agents_enabled = false;
    config
        .features
        .enable(Feature::DeferredToolWorldState)
        .expect("test config should allow feature update");
}

fn tools_state_sections(request: &ResponsesRequest) -> Vec<String> {
    request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("<tools>"))
        .collect()
}

fn completed_response_sequence(count: usize) -> Vec<String> {
    (1..=count)
        .map(|index| {
            sse(vec![
                ev_response_created(&format!("resp-{index}")),
                ev_assistant_message(&format!("msg-{index}"), "done"),
                ev_completed(&format!("resp-{index}")),
            ])
        })
        .collect()
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
async fn deferred_tool_world_state_is_disabled_by_default() -> Result<()> {
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

    let mut builder = search_capable_apps_builder(apps_server.chatgpt_base_url.clone());
    let test = builder.build(&server).await?;
    test.submit_turn("inspect deferred MCP tools").await?;

    let request = response.single_request();
    assert!(
        request
            .message_input_texts("developer")
            .into_iter()
            .all(|text| !text.contains("<tools>")),
        "deferred tool world state should not be injected unless its feature is enabled"
    );
    assert!(
        request.body_json()["tools"]
            .as_array()
            .is_some_and(|tools| {
                tools
                    .iter()
                    .any(|tool| tool.get("type").and_then(Value::as_str) == Some("tool_search"))
            }),
        "disabling tool world state must not disable deferred tool discovery"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deferred_tool_world_state_tracks_initial_unchanged_and_removed_namespaces() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let apps_server = AppsTestServer::mount_searchable(&server).await?;
    let response = mount_sse_sequence(&server, completed_response_sequence(/*count*/ 3)).await;
    let mut builder = search_capable_apps_builder(apps_server.chatgpt_base_url.clone())
        .with_config(enable_deferred_tool_world_state_without_agents);
    let test = builder.build_with_auto_env(&server).await?;
    wait_for_mcp_server(&test.codex, CODEX_APPS_MCP_SERVER_NAME).await?;

    test.submit_turn("inspect initially available deferred tools")
        .await?;
    test.submit_turn("inspect unchanged deferred tools").await?;

    let mut refresh_config = test.config.clone();
    let user_config_path = refresh_config.codex_home.join("config.toml");
    let user_config = toml::from_str(
        r#"
[apps.calendar]
enabled = false
"#,
    )?;
    refresh_config.config_layer_stack = refresh_config
        .config_layer_stack
        .with_user_config(&user_config_path, user_config)?;
    test.codex.refresh_runtime_config(refresh_config).await;
    test.codex.submit(Op::RefreshMcpServers).await?;
    test.submit_turn("inspect removed deferred tools").await?;

    let requests = response.requests();
    assert_eq!(requests.len(), 3);
    let tools_states = requests
        .iter()
        .map(tools_state_sections)
        .collect::<Vec<_>>();
    assert_eq!(tools_states[0], tools_states[1]);
    assert_eq!(tools_states[2].len(), 2);
    assert!(tools_states[2][1].contains("Removed deferred tool namespaces:\n"));
    assert!(tools_states[2][1].contains("No deferred tool namespaces remain.\n"));
    insta::assert_snapshot!(
        "deferred_tools_initial_unchanged_and_removed",
        format_labeled_requests_snapshot(
            "Initially available deferred tools remain unchanged until disabling Calendar removes them.",
            &[
                ("Initially available", &requests[0]),
                ("Unchanged follow-up", &requests[1]),
                ("Removed and empty", &requests[2]),
            ],
        )
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initially_empty_deferred_tool_world_state_is_not_rendered_or_persisted() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response = mount_sse_sequence(&server, completed_response_sequence(/*count*/ 1)).await;
    let mut builder = core_test_support::test_codex::test_codex()
        .with_config(enable_deferred_tool_world_state_without_agents);
    let test = builder.build_with_auto_env(&server).await?;
    test.submit_turn("inspect empty deferred tools").await?;

    let request = response.single_request();
    assert!(tools_state_sections(&request).is_empty());
    test.codex.ensure_rollout_materialized().await;
    test.codex.flush_rollout().await?;
    let rollout_path = test.codex.rollout_path().expect("rollout path");
    let world_states = tokio::fs::read_to_string(rollout_path)
        .await?
        .lines()
        .map(serde_json::from_str::<RolloutLine>)
        .collect::<serde_json::Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::WorldState(item) => Some(item.state),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!world_states.is_empty());
    assert!(
        world_states
            .iter()
            .all(|state| state.get("tools").is_none())
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deferred_tool_world_state_survives_resume_without_duplicate_updates() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let apps_server = AppsTestServer::mount_searchable(&server).await?;
    let response = mount_sse_sequence(&server, completed_response_sequence(/*count*/ 2)).await;
    let mut builder = search_capable_apps_builder(apps_server.chatgpt_base_url.clone())
        .with_config(enable_deferred_tool_world_state_without_agents);
    let initial = builder.build_with_auto_env(&server).await?;
    wait_for_mcp_server(&initial.codex, CODEX_APPS_MCP_SERVER_NAME).await?;
    initial
        .submit_turn("inspect deferred tools before resume")
        .await?;

    initial.codex.ensure_rollout_materialized().await;
    initial.codex.flush_rollout().await?;
    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");
    let persisted_tools = tokio::fs::read_to_string(&rollout_path)
        .await?
        .lines()
        .map(serde_json::from_str::<RolloutLine>)
        .collect::<serde_json::Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::WorldState(item) => item.state.get("tools").cloned(),
            _ => None,
        })
        .next_back()
        .expect("rollout should persist the deferred tools world state");
    assert_eq!(
        persisted_tools,
        json!({
            "mcp__codex_apps__calendar": "Plan events and manage your calendar."
        })
    );
    drop(initial);

    let mut resume_builder = search_capable_apps_builder(apps_server.chatgpt_base_url)
        .with_config(enable_deferred_tool_world_state_without_agents);
    let resumed = resume_builder.resume(&server, home, rollout_path).await?;
    wait_for_mcp_server(&resumed.codex, CODEX_APPS_MCP_SERVER_NAME).await?;
    resumed
        .submit_turn("inspect unchanged deferred tools after resume")
        .await?;

    let requests = response.requests();
    assert_eq!(requests.len(), 2);
    let tools_states = requests
        .iter()
        .map(tools_state_sections)
        .collect::<Vec<_>>();
    assert_eq!(tools_states[0], tools_states[1]);
    assert_eq!(tools_states[0].len(), 1);
    assert!(tools_states[0][0].contains(SEARCH_CALENDAR_NAMESPACE));
    insta::assert_snapshot!(
        "deferred_tools_resume_without_duplicate_update",
        format_labeled_requests_snapshot(
            "Persisted deferred tools remain unchanged after resuming the thread.",
            &[
                ("Before resume", &requests[0]),
                ("After resume", &requests[1]),
            ],
        )
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apps_guidance_and_deferred_namespace_appear_after_recovery_within_a_turn() -> Result<()> {
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
    let mut builder = search_capable_apps_builder(apps_server.chatgpt_base_url.clone())
        .with_config(|config| {
            config
                .features
                .enable(Feature::DefaultModeRequestUserInput)
                .expect("test config should allow feature update");
            config
                .features
                .enable(Feature::DeferredToolWorldState)
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
    let initial_tools_state = initial_request
        .message_input_texts("developer")
        .into_iter()
        .find(|text| text.contains("<tools>"))
        .expect("initial request should contain tools world state");
    assert!(
        !initial_tools_state.contains(SEARCH_CALENDAR_NAMESPACE),
        "Calendar namespace should not be advertised before recovery: {initial_tools_state}"
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
    let recovered_tools_state = requests[1]
        .message_input_texts("developer")
        .into_iter()
        .find(|text| text.contains("Added deferred tool namespaces:"))
        .expect("recovered request should contain a tools world-state delta");
    assert!(
        recovered_tools_state.contains(&format!(
            "- {SEARCH_CALENDAR_NAMESPACE}: Plan events and manage your calendar."
        )),
        "Calendar namespace and description should be added after recovery: {recovered_tools_state}"
    );
    let recovered_body = requests[1].body_json();
    assert!(
        namespace_child_tool(
            &recovered_body,
            SEARCH_CALENDAR_NAMESPACE,
            SEARCH_CALENDAR_CREATE_TOOL,
        )
        .is_none(),
        "deferred Calendar namespace should not be directly advertised: {recovered_body}"
    );
    assert!(
        recovered_body["tools"].as_array().is_some_and(|tools| {
            tools
                .iter()
                .any(|tool| tool.get("type").and_then(Value::as_str) == Some("tool_search"))
        }),
        "recovered request should advertise tool_search: {recovered_body}"
    );
    insta::assert_snapshot!(
        "deferred_tools_recover_during_sampling",
        format_labeled_requests_snapshot(
            "Deferred namespaces appear after Apps recovers between sampling requests.",
            &[
                ("Apps unavailable", &requests[0]),
                ("Apps recovered", &requests[1]),
            ],
        )
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
