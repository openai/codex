use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::X_CODEX_ROUTING_HINT_HEADER;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::openai_models::ModelServiceTier;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::WebSocketConnectionConfig;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_exec_command_call;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::start_websocket_server;
use core_test_support::responses::start_websocket_server_with_headers;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;

const WS_V2_BETA_HEADER_VALUE: &str = "responses_websockets=2026-02-06";

#[test_case::test_case(true; "fast mode enabled")]
#[test_case::test_case(false; "fast mode disabled")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawned_agent_prewarm_handshake_inherits_effective_root_service_tier(
    fast_mode_enabled: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![
        vec![
            vec![ev_response_created("root-warm"), ev_completed("root-warm")],
            vec![
                ev_response_created("root-spawn"),
                ev_function_call_with_namespace(
                    "spawn-worker",
                    "collaboration",
                    "spawn_agent",
                    &json!({ "message": "hello", "task_name": "worker", "fork_turns": "none" })
                        .to_string(),
                ),
                ev_completed("root-spawn"),
            ],
            vec![ev_response_created("root-done"), ev_completed("root-done")],
        ],
        vec![
            vec![
                ev_response_created("child-warm"),
                ev_completed("child-warm"),
            ],
            vec![
                ev_response_created("child-done"),
                ev_completed("child-done"),
            ],
        ],
    ])
    .await;
    let tier = ServiceTier::Fast.request_value();
    let test = test_codex()
        .with_model("gpt-5.4")
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model_info_override("gpt-5.4", move |model| {
            model.service_tiers.push(ModelServiceTier {
                id: tier.to_string(),
                name: "Fast".to_string(),
                description: "Priority processing".to_string(),
            });
        })
        .with_config(move |config| {
            for feature in [Feature::Collab, Feature::MultiAgentV2] {
                config.features.enable(feature).expect("spawn feature");
            }
            if fast_mode_enabled {
                config
                    .features
                    .enable(Feature::FastMode)
                    .expect("fast mode");
            } else {
                config
                    .features
                    .disable(Feature::FastMode)
                    .expect("fast mode");
            }
            config.service_tier = Some(ServiceTier::Fast.request_value().to_string());
        })
        .build_with_websocket_server(&server)
        .await?;
    let mut created_threads = test.thread_manager.subscribe_thread_created();
    tokio::time::timeout(
        Duration::from_secs(5),
        server.wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
    )
    .await?;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "spawn a worker".to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                service_tier: Some(Some(tier.to_string())),
                ..Default::default()
            }),
        )
        .await?;
    let child_thread_id =
        tokio::time::timeout(Duration::from_secs(5), created_threads.recv()).await??;
    let child = test.thread_manager.get_thread(child_thread_id).await?;

    assert!(
        server
            .wait_for_handshakes(/*expected*/ 2, Duration::from_secs(5))
            .await,
        "child startup should connect before its first turn"
    );
    let handshakes = server.handshakes();
    let expected_hint = if fast_mode_enabled {
        format!("model=gpt-5.4;tier={tier}")
    } else {
        "model=gpt-5.4".to_string()
    };
    assert_eq!(
        handshakes[1].header(X_CODEX_ROUTING_HINT_HEADER),
        Some(expected_hint)
    );
    let child_warmup = tokio::time::timeout(
        Duration::from_secs(5),
        server.wait_for_request(/*connection_index*/ 1, /*request_index*/ 0),
    )
    .await?
    .body_json();
    assert_eq!(child_warmup["generate"], false);
    assert_eq!(
        child_warmup["service_tier"].as_str(),
        fast_mode_enabled.then_some(tier)
    );
    assert_eq!(server.handshakes().len(), 2);
    child.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_model_switch_to_responses_lite_omits_top_level_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("warm-1"), ev_completed("warm-1")],
        vec![ev_response_created("resp-1"), ev_completed("resp-1")],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;

    let mut builder = test_codex()
        .with_model_info_override("gpt-5.2", |model_info| {
            model_info.tool_mode = Some(ToolMode::CodeMode);
            model_info.node_repl_auto_review_required = true;
        })
        .with_model_info_override("gpt-5.4", |model_info| {
            model_info.use_responses_lite = true;
            model_info.tool_mode = Some(ToolMode::CodeMode);
            model_info.node_repl_disabled = true;
        })
        .with_model("gpt-5.2");
    let test = builder.build_with_websocket_server(&server).await?;

    test.submit_turn("non-lite turn").await?;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "lite turn".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                model: Some("gpt-5.4".to_string()),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    assert_eq!(server.handshakes().len(), 1);
    let connection = server.single_connection();
    assert_eq!(connection.len(), 3);
    let non_lite_turn = connection
        .get(1)
        .expect("missing non-lite turn request")
        .body_json();
    let lite_turn = connection
        .get(2)
        .expect("missing lite turn request")
        .body_json();

    assert_eq!(non_lite_turn["model"].as_str(), Some("gpt-5.2"));
    assert_eq!(lite_turn["model"].as_str(), Some("gpt-5.4"));
    for (request, auto_review_required, disabled) in
        [(&non_lite_turn, true, false), (&lite_turn, false, true)]
    {
        let metadata: Value = serde_json::from_str(
            request["client_metadata"]["x-codex-turn-metadata"]
                .as_str()
                .expect("websocket request should include turn metadata"),
        )?;
        assert_eq!(
            metadata["node_repl_auto_review_required"],
            auto_review_required
        );
        assert_eq!(metadata["node_repl_disabled"], disabled);
    }
    assert!(
        non_lite_turn
            .get("tools")
            .and_then(Value::as_array)
            .is_some_and(|tools| !tools.is_empty())
    );
    assert_eq!(lite_turn.get("previous_response_id"), None);
    assert_eq!(lite_turn.get("tools"), None);
    assert_eq!(lite_turn.get("instructions"), None);
    let additional_tools = lite_turn
        .get("input")
        .and_then(Value::as_array)
        .and_then(|input| input.first())
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("additional_tools"))
        .and_then(|item| item.get("tools"))
        .and_then(Value::as_array)
        .expect("lite turn should start with an additional_tools item");
    assert!(!additional_tools.is_empty());

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_test_codex_shell_chain() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let call_id = "exec-command-call";
    let server = start_websocket_server(vec![vec![
        vec![
            ev_response_created("resp-1"),
            ev_exec_command_call(call_id, "echo websocket"),
            ev_completed("resp-1"),
        ],
        vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg_1", "done"),
            ev_completed("resp-2"),
        ],
    ]])
    .await;

    let mut builder = test_codex().with_windows_cmd_shell();

    let test = builder.build_with_websocket_server(&server).await?;
    test.submit_turn_with_policy("run the echo command", test.config.legacy_sandbox_policy())
        .await?;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);

    let first_turn = connection
        .first()
        .expect("missing first turn request")
        .body_json();
    let second_turn = connection
        .get(1)
        .expect("missing second turn request")
        .body_json();

    assert_eq!(first_turn["type"].as_str(), Some("response.create"));
    assert_eq!(second_turn["type"].as_str(), Some("response.create"));

    let input_items = second_turn
        .get("input")
        .and_then(Value::as_array)
        .expect("second response.create input array");
    assert!(!input_items.is_empty());

    server.shutdown().await;
    Ok(())
}

#[test_case::test_case(false; "update_plan disabled")]
#[test_case::test_case(true; "update_plan enabled")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_first_turn_uses_startup_prewarm_and_create(
    update_plan_enabled: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("warm-1"), ev_completed("warm-1")],
        vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg_1", "hello"),
            ev_completed("resp-1"),
        ],
    ]])
    .await;

    let mut builder = test_codex()
        .with_model("gpt-5.2")
        .with_config(move |config| {
            config.update_plan_enabled = update_plan_enabled;
            config.analytics_enabled = Some(false);
        });
    let test = builder.build_with_websocket_server(&server).await?;
    test.submit_turn_with_policy("hello", test.config.legacy_sandbox_policy())
        .await?;

    assert_eq!(server.handshakes().len(), 1);
    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);
    let warmup = connection
        .first()
        .expect("missing warmup request")
        .body_json();
    let turn = connection.get(1).expect("missing turn request").body_json();
    assert_eq!(warmup["instructions"], turn["instructions"]);
    assert_eq!(
        warmup["instructions"]
            .as_str()
            .expect("warmup base instructions")
            .contains("update_plan"),
        update_plan_enabled
    );
    assert_eq!(warmup["type"].as_str(), Some("response.create"));
    assert_eq!(warmup["generate"].as_bool(), Some(false));
    let warmup_metadata: Value = serde_json::from_str(
        warmup["client_metadata"]["x-codex-turn-metadata"]
            .as_str()
            .expect("warmup turn metadata"),
    )?;
    assert_eq!(warmup_metadata["request_kind"].as_str(), Some("prewarm"));
    assert_eq!(warmup_metadata["analytics_enabled"].as_bool(), Some(false));
    assert_eq!(
        warmup_metadata["window_id"].as_str(),
        warmup["client_metadata"]["x-codex-window-id"].as_str()
    );
    assert!(
        turn["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty()),
        "expected request tools to be populated"
    );
    assert_eq!(turn["type"].as_str(), Some("response.create"));
    assert_eq!(turn.get("generate"), None);
    let turn_metadata: Value = serde_json::from_str(
        turn["client_metadata"]["x-codex-turn-metadata"]
            .as_str()
            .expect("turn metadata"),
    )?;
    assert_eq!(turn_metadata["request_kind"].as_str(), Some("turn"));
    assert_eq!(turn_metadata["analytics_enabled"].as_bool(), Some(false));
    assert_eq!(warmup_metadata["window_number"].as_u64(), Some(0));
    assert_eq!(
        warmup_metadata["window_number"],
        turn_metadata["window_number"]
    );
    assert!(warmup_metadata["context_window_id"].is_string());
    assert_eq!(
        warmup_metadata["context_window_id"],
        turn_metadata["context_window_id"]
    );

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_first_turn_handles_handshake_delay_with_startup_prewarm() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server_with_headers(vec![WebSocketConnectionConfig {
        requests: vec![
            vec![ev_response_created("warm-1"), ev_completed("warm-1")],
            vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg_1", "hello"),
                ev_completed("resp-1"),
            ],
        ],
        response_headers: Vec::new(),
        // Delay handshake so turn processing must tolerate websocket startup latency.
        accept_delay: Some(Duration::from_millis(150)),
        close_after_requests: true,
    }])
    .await;

    let mut builder = test_codex();
    let test = builder.build_with_websocket_server(&server).await?;
    test.submit_turn_with_policy("hello", test.config.legacy_sandbox_policy())
        .await?;

    assert_eq!(server.handshakes().len(), 1);
    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);
    let warmup = connection
        .first()
        .expect("missing warmup request")
        .body_json();
    let turn = connection.get(1).expect("missing turn request").body_json();
    assert_eq!(warmup["type"].as_str(), Some("response.create"));
    assert_eq!(warmup["generate"].as_bool(), Some(false));
    assert!(
        turn["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty()),
        "expected request tools to be populated"
    );
    assert_eq!(turn["type"].as_str(), Some("response.create"));

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_v2_test_codex_shell_chain() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let call_id = "exec-command-call";
    let mut exec_command_call = ev_exec_command_call(call_id, "echo websocket");
    exec_command_call["item"]["id"] = serde_json::json!("fc_exec_command_call");
    exec_command_call["item"]["internal_chat_message_metadata_passthrough"] =
        serde_json::json!({"turn_id": "turn-123"});
    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("warm-1"), ev_completed("warm-1")],
        vec![
            ev_response_created("resp-1"),
            exec_command_call,
            ev_completed("resp-1"),
        ],
        vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg_1", "done"),
            ev_completed("resp-2"),
        ],
    ]])
    .await;

    let mut builder = test_codex().with_windows_cmd_shell().with_config(|config| {
        config
            .features
            .enable(Feature::ResponsesWebsocketsV2)
            .expect("test config should allow feature update");
    });

    let test = builder.build_with_websocket_server(&server).await?;
    test.submit_turn_with_policy("run the echo command", test.config.legacy_sandbox_policy())
        .await?;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 3);

    let warmup = connection
        .first()
        .expect("missing warmup request")
        .body_json();
    let first_turn = connection
        .get(1)
        .expect("missing first turn request")
        .body_json();
    let second_turn = connection
        .get(2)
        .expect("missing second turn request")
        .body_json();

    assert_eq!(warmup["type"].as_str(), Some("response.create"));
    assert_eq!(warmup["generate"].as_bool(), Some(false));
    assert_eq!(first_turn["type"].as_str(), Some("response.create"));
    assert_eq!(first_turn["previous_response_id"].as_str(), Some("warm-1"));
    assert!(
        first_turn
            .get("input")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
    );
    assert_eq!(second_turn["type"].as_str(), Some("response.create"));
    assert_eq!(second_turn["previous_response_id"].as_str(), Some("resp-1"));

    let create_items = second_turn
        .get("input")
        .and_then(Value::as_array)
        .expect("response.create input array");
    assert!(!create_items.is_empty());

    let output_item = create_items
        .iter()
        .find(|item| item.get("type").and_then(Value::as_str) == Some("function_call_output"))
        .expect("function_call_output in create");
    assert_eq!(
        output_item.get("call_id").and_then(Value::as_str),
        Some(call_id)
    );

    let handshake = server.single_handshake();
    assert_eq!(
        handshake.header("openai-beta"),
        Some(WS_V2_BETA_HEADER_VALUE.to_string())
    );

    server.shutdown().await;
    Ok(())
}

#[test_case::test_case(None, Some("ultrafast"); "standard to ultrafast")]
#[test_case::test_case(Some("ultrafast"), None; "ultrafast to standard")]
#[test_case::test_case(Some("priority"), Some("ultrafast"); "priority to ultrafast")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_v2_first_turn_sends_full_create_when_tier_changes_after_startup_prewarm(
    startup_tier: Option<&'static str>,
    turn_tier: Option<&'static str>,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let warmup_turn_state = "startup-turn-state";
    let server = start_websocket_server(vec![vec![
        vec![
            ev_response_created("warm-1"),
            json!({
                "type": "response.metadata",
                "headers": {"x-codex-turn-state": warmup_turn_state},
            }),
            ev_completed("warm-1"),
        ],
        vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg_1", "done"),
            ev_completed("resp-1"),
        ],
    ]])
    .await;

    let model = "gpt-5.6-sol";
    let mut builder = test_codex()
        .with_model_info_override(model, |model_info| {
            // Exercise tier transitions independently of the bundled model catalog.
            model_info.service_tiers = ["priority", "ultrafast"]
                .into_iter()
                .map(|tier| ModelServiceTier {
                    id: tier.to_string(),
                    name: tier.to_string(),
                    description: String::new(),
                })
                .collect();
        })
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            for feature in [Feature::ResponsesWebsocketsV2, Feature::FastMode] {
                config
                    .features
                    .enable(feature)
                    .expect("test config should allow feature update");
            }
            config.service_tier = startup_tier.map(str::to_string);
        });
    let test = builder.build_with_websocket_server(&server).await?;

    let warmup = server
        .wait_for_request(/*connection_index*/ 0, /*request_index*/ 0)
        .await
        .body_json();
    assert_eq!(warmup["type"].as_str(), Some("response.create"));
    assert_eq!(warmup["generate"].as_bool(), Some(false));
    assert_eq!(warmup["service_tier"].as_str(), startup_tier);

    test.submit_turn_with_service_tier("hello", turn_tier)
        .await?;

    assert_eq!(server.handshakes().len(), 1);
    assert_eq!(
        server
            .single_handshake()
            .header(X_CODEX_ROUTING_HINT_HEADER),
        Some(match startup_tier {
            Some(tier) => format!("model={model};tier={tier}"),
            None => format!("model={model}"),
        })
    );
    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);
    let first_turn = connection
        .get(1)
        .expect("missing first turn request")
        .body_json();

    assert_eq!(first_turn["type"].as_str(), Some("response.create"));
    assert_eq!(first_turn["model"].as_str(), Some(model));
    assert_eq!(first_turn["service_tier"].as_str(), turn_tier);
    assert_ne!(first_turn["generate"].as_bool(), Some(false));
    assert_eq!(
        first_turn["client_metadata"]["x-codex-turn-state"].as_str(),
        Some(warmup_turn_state)
    );
    assert_eq!(first_turn.get("previous_response_id"), None);
    assert!(
        first_turn
            .get("input")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
    );

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websocket_v2_next_turn_uses_updated_service_tier() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("warm-1"), ev_completed("warm-1")],
        vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg_1", "fast"),
            ev_completed("resp-1"),
        ],
        vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg_2", "standard"),
            ev_completed("resp-2"),
        ],
    ]])
    .await;

    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::ResponsesWebsocketsV2)
            .expect("test config should allow feature update");
    });
    let test = builder.build_with_websocket_server(&server).await?;

    let warmup = server
        .wait_for_request(/*connection_index*/ 0, /*request_index*/ 0)
        .await
        .body_json();
    assert_eq!(warmup["type"].as_str(), Some("response.create"));
    assert_eq!(warmup["generate"].as_bool(), Some(false));
    assert_eq!(warmup.get("service_tier"), None);

    test.submit_turn_with_service_tier("first", Some(ServiceTier::Fast.request_value()))
        .await?;
    test.submit_turn_with_service_tier("second", /*service_tier*/ None)
        .await?;

    assert_eq!(server.handshakes().len(), 1);
    let connection = server.single_connection();
    assert_eq!(connection.len(), 3);

    let first_turn = connection
        .get(1)
        .expect("missing first turn request")
        .body_json();
    let second_turn = connection
        .get(2)
        .expect("missing second turn request")
        .body_json();

    assert_eq!(first_turn["type"].as_str(), Some("response.create"));
    assert_eq!(first_turn["service_tier"].as_str(), Some("priority"));
    assert_eq!(first_turn.get("previous_response_id"), None);
    assert!(
        first_turn
            .get("input")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
    );

    assert_eq!(second_turn["type"].as_str(), Some("response.create"));
    assert_eq!(second_turn.get("service_tier"), None);
    assert_eq!(second_turn.get("previous_response_id"), None);
    assert!(
        second_turn
            .get("input")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
    );

    server.shutdown().await;
    Ok(())
}
