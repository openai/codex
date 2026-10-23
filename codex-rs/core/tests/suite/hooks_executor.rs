use anyhow::Context;
use anyhow::Result;
use codex_config::McpServerConfig;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::EnvironmentConfig;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_core::windows_sandbox::WindowsSandboxLevelExt;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecServerRuntimeOptions;
use codex_features::Feature;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfileSnapshot;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::ThreadSource;
use codex_protocol::protocol::TurnEnvironmentRequest;
use codex_protocol::protocol::TurnEnvironmentRequests;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use codex_utils_path_uri::PathUri;
use core_test_support::TestTargetOs;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::apps_enabled_builder;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::exec_server::ExecServerProcess;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_response_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_remote;
use core_test_support::skip_if_wine_exec;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::environment_config_for_selection;
use core_test_support::test_codex::test_codex;
use core_test_support::test_target_os;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use test_case::test_case;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::rmcp_client::remote_aware_environment_id;
use super::rmcp_client::remote_aware_stdio_server_bin;

#[test_case(
    json!({"permissionDecision": "deny", "permissionDecisionReason": "blocked by owner"}),
    "blocked by owner"; "camelCase environment selects the owners blocking hook"
)]
#[test_case(
    json!({"permissionDecision": "allow", "updatedInput": {"permissions": {"network": {"enabled": true}}}}),
    "\"scope\":\"turn\""; "rewrite may omit the original environment"
)]
#[test_case(
    json!({"permissionDecision": "allow", "updatedInput": {"environmentId": "another", "permissions": {"network": {"enabled":true}}}}),
    "PreToolUse hook cannot change the tool environment"; "rewrite cannot change the original environment"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_tool_use_preserves_request_permissions_environment(
    hook_output: Value,
    expected_message: &str,
) -> Result<()> {
    let server = start_mock_server().await;
    let test = test_codex()
        .with_config(|config| {
            for feature in [Feature::CodexHooks, Feature::RequestPermissionsTool] {
                config.features.enable(feature).expect("enable feature");
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let (selections, _executor) = environments_with_pre_tool_use_hook(
        &test,
        HookEnvironment::Secondary,
        "^request_permissions$",
        hook_output,
    )
    .await?;
    let args = json!({"environmentId": selections[1].environment_id, "permissions": {"network": {"enabled": true}}});
    let responses = mount_response_sequence(
        &server,
        vec![
            sse_response(sse(vec![
                ev_function_call("permissions", "request_permissions", &args.to_string()),
                ev_completed("first"),
            ])),
            completed_turn_response("second"),
        ],
    )
    .await;
    test.submit_turn_with_environments("request network access", Some(selections))
        .await?;
    let output = responses
        .function_call_output_text("permissions")
        .context("expected request_permissions result")?;
    assert!(output.contains(expected_message), "{output}");
    Ok(())
}

#[test_case(HookEnvironment::Secondary; "unrelated environment cannot block a targeted resource call")]
#[test_case(HookEnvironment::Primary; "server owner can block a targeted resource call")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_resource_hooks_follow_server_ownership(
    hook_environment: HookEnvironment,
) -> Result<()> {
    skip_if_wine_exec!(Ok(()), "requires a Windows test_stdio_server binary");
    let server = start_mock_server().await;
    let command = remote_aware_stdio_server_bin()?;
    let test = test_codex()
        .with_config(move |config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("enable hooks");
            let mcp_server: McpServerConfig = serde_json::from_value(json!({
                "command": command,
                "environment_id": remote_aware_environment_id(),
                "cwd": config.cwd,
            }))
            .expect("valid docs MCP server configuration");
            config
                .mcp_servers
                .set(HashMap::from([("docs".to_string(), mcp_server)]))
                .expect("configure docs MCP server");
        })
        .build_with_auto_env(&server)
        .await?;
    wait_for_mcp_server(&test.codex, "docs").await?;

    let denied = "denied by resource hook";
    let expected = match hook_environment {
        HookEnvironment::Primary => denied,
        HookEnvironment::Secondary => "memo://codex/",
    };
    let (mut selections, _executor) = environments_with_pre_tool_use_hook(
        &test,
        hook_environment,
        "^(read|list)_mcp_resource",
        json!({"permissionDecision": "deny", "permissionDecisionReason": denied}),
    )
    .await?;
    // Put the server's environment second so routing every call to the primary fails this test too.
    selections.reverse();
    let read = json!({"server": "docs", "uri": "memo://codex/example-note"});
    let list = json!({"server": " docs "});
    let calls = [
        ("read", "read_mcp_resource", read, expected),
        ("list", "list_mcp_resources", list.clone(), expected),
        ("templates", "list_mcp_resource_templates", list, expected),
        ("list-all", "list_mcp_resources", json!({}), denied),
        (
            "unknown",
            "list_mcp_resources",
            json!({"server": "missing"}),
            "unknown MCP server 'missing'",
        ),
    ];
    let mut events = vec![ev_response_created("resources")];
    events.extend(
        calls
            .iter()
            .map(|(id, tool, args, _)| ev_function_call(id, tool, &args.to_string())),
    );
    events.push(ev_completed("resources"));
    let responses = mount_response_sequence(
        &server,
        vec![sse_response(sse(events)), completed_turn_response("done")],
    )
    .await;
    test.submit_turn_with_environments("inspect MCP resources", Some(selections))
        .await?;
    for (id, _, _, expected) in calls {
        let output = responses
            .function_call_output_text(id)
            .context("missing resource tool result")?;
        assert!(
            output.contains(expected),
            "{id}: expected {expected:?} in {output}"
        );
    }
    Ok(())
}

enum HookEnvironment {
    Primary,
    Secondary,
}

async fn environments_with_pre_tool_use_hook(
    test: &TestCodex,
    hook_environment: HookEnvironment,
    matcher: &str,
    mut hook_output: Value,
) -> Result<(Vec<TurnEnvironmentSelection>, Option<ExecServerProcess>)> {
    let mut executor = None;
    let executor_url = if let Some(url) = test.executor_environment().exec_server_url() {
        url
    } else {
        &executor
            .insert(ExecServerProcess::start().await?)
            .websocket_url
    };
    let mut primary = test.executor_environment().selection().clone();
    let mut secondary = primary.clone();
    secondary.environment_id = "secondary".to_string();
    test.thread_manager
        .environment_manager()
        .upsert_environment(
            secondary.environment_id.clone(),
            executor_url.to_owned(),
            /*connect_timeout*/ None,
        )?;

    let owner = match hook_environment {
        HookEnvironment::Primary => &mut primary,
        HookEnvironment::Secondary => &mut secondary,
    };
    hook_output["hookEventName"] = json!("PreToolUse");
    let output = json!({"hookSpecificOutput": hook_output});
    let command = match test_target_os() {
        TestTargetOs::Windows => format!("ForEach-Object {{ '{output}' }}"),
        TestTargetOs::Linux | TestTargetOs::MacOs => format!("printf '%s' '{output}'"),
    };
    let mut config = environment_config_for_selection(&test.config, owner);
    config.environment_hooks = serde_json::from_value(json!({
        "PreToolUse": [{
            "matcher": matcher,
            "hooks": [{"type": "command", "command": command}],
        }],
    }))?;
    owner.config = EnvironmentConfigState::Ready(config);
    Ok((vec![primary, secondary], executor))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn environment_requires_loadable_managed_hooks_at_startup_and_readiness() -> Result<()> {
    let server = start_mock_server().await;
    let test = test_codex()
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("enable hooks");
            config
                .features
                .enable(Feature::DeferredExecutor)
                .expect("allow the environment to start with pending configuration");
        })
        .build_with_auto_env(&server)
        .await?;
    let mut selection = test.executor_environment().selection().clone();
    let mut config = environment_config_for_selection(&test.config, &selection);
    config.environment_hooks = toml::from_str(
        r#"
        [[PreToolUse]]
        matcher = "["
        [[PreToolUse.hooks]]
        type = "command"
        command = "echo managed-hook"
        "#,
    )?;
    selection.config = EnvironmentConfigState::Ready(config.clone());
    let start = |selection: TurnEnvironmentSelection| {
        test.thread_manager.start_thread(StartThreadOptions {
            environments: Some(vec![selection.into_request()]),
            ..StartThreadOptions::new(test.config.clone())
        })
    };
    let assert_invalid_matcher = |error: String| {
        assert!(error.contains("invalid matcher"), "{error}");
        assert!(error.contains(&selection.environment_id), "{error}");
    };
    let error = start(selection.clone())
        .await
        .err()
        .context("startup accepted an invalid managed hook")?;
    assert_invalid_matcher(error.to_string());

    selection.config = EnvironmentConfigState::Pending;
    let thread = start(selection.clone()).await?.thread;
    let error = thread
        .environment_ready(&selection, config.clone())
        .await
        .expect_err("an invalid managed hook must prevent readiness");
    assert_invalid_matcher(error.to_string());
    assert_eq!(
        thread.environment_selections().await,
        vec![selection.clone()]
    );

    config.environment_hooks.pre_tool_use[0].matcher = Some("^Bash$".to_string());
    thread.environment_ready(&selection, config).await?;
    assert!(
        thread
            .config_snapshot()
            .await
            .is_primary_environment_configured()
    );
    thread.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn thread_plugin_selection_disables_executor_hooks_without_disabling_their_server()
-> Result<()> {
    skip_if_no_network!(Ok(()));
    let fixture = executor_hook_fixture(
        ["disabled", "enabled", "disabled-again"]
            .map(completed_turn_response)
            .to_vec(),
    )
    .await?;
    fixture.attach().await?;
    for (disabled_plugin_ids, expected_calls) in [
        (vec!["computer-use@openai-bundled".to_string()], 0),
        (Vec::new(), 1),
        (vec!["computer-use@openai-bundled".to_string()], 1),
    ] {
        let enabled = disabled_plugin_ids.is_empty();
        submit_thread_settings(
            &fixture.test.codex,
            ThreadSettingsOverrides {
                disabled_plugin_ids: Some(disabled_plugin_ids),
                ..Default::default()
            },
        )
        .await?;
        fixture.test.submit_text_turn("finish this turn").await?;
        if enabled {
            fixture.wait_for_hook_call().await?;
        }
        assert_eq!(fixture.calls().await?.len(), expected_calls);
    }
    fixture
        .test
        .codex
        .call_mcp_tool(
            "node_repl",
            "js",
            Some(json!({"code": "1 + 1"})),
            /*meta*/ None,
        )
        .await?;
    assert_eq!(
        fixture
            .calls()
            .await?
            .last()
            .context("standalone tool call")?["params"]["name"],
        "js"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn executor_stop_hook_uses_plugin_policy_refreshed_during_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let fixture = executor_plugin_hook_fixture(
        test_codex().with_config(|config| {
            config
                .features
                .enable(Feature::DefaultModeRequestUserInput)
                .expect("enable request_user_input");
        }),
        &[("computer-use@openai-bundled", computer_use_hook_manifest())],
        vec![
            sse_response(sse(vec![
                ev_function_call(
                    "pause",
                    "request_user_input",
                    r#"{"questions":[{"id":"continue","header":"Continue","question":"Continue?","options":[{"label":"Yes","description":"Finish."},{"label":"No","description":"Stop."}]}]}"#,
                ),
                ev_completed("paused-step"),
            ])),
            completed_turn_response("finished-turn"),
        ],
    )
    .await?;
    fixture.attach().await?;
    fixture
        .test
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Ask before finishing this turn".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let request = wait_for_event_match(&fixture.test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;

    let current_config = fixture.test.codex.config().await;
    let mut config = (*current_config).clone();
    config.config_layer_stack = config.config_layer_stack.with_user_config(
        &config.codex_home.join("config.toml"),
        toml::from_str("[plugins._default]\nenabled = false")?,
    )?;
    let _ = fixture
        .test
        .codex
        .refresh_runtime_config(current_config, config)
        .await;
    fixture
        .test
        .codex
        .submit(Op::UserInputAnswer {
            id: request.turn_id,
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    "continue".to_string(),
                    RequestUserInputAnswer {
                        answers: vec!["Yes".to_string()],
                    },
                )]),
            },
        })
        .await?;
    wait_for_event(&fixture.test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    assert_eq!(fixture.responses.requests().len(), 2);
    assert_eq!(fixture.calls().await?, Vec::<Value>::new());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn executor_stop_hook_runs_after_attachment() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let fixture = executor_stop_hook_fixture().await?;
    fixture
        .test
        .submit_text_turn("before the executor plugin attaches")
        .await?;
    assert_eq!(fixture.calls().await?, Vec::<Value>::new());

    fixture.attach().await?;
    fixture
        .test
        .submit_text_turn("after the executor plugin attaches")
        .await?;
    fixture.wait_for_hook_call().await?;

    let calls = fixture.calls().await?;
    assert_eq!(calls.len(), 1);
    let call = &calls[0];
    let turn_metadata = &call["params"]["_meta"]["x-codex-turn-metadata"];
    let response_body = fixture.responses.requests()[1].body_json();
    assert_eq!(call["params"]["name"], "turn_ended");
    assert_eq!(call["params"]["arguments"]["hook_event_name"], "Stop");
    assert_eq!(
        call["params"]["arguments"]["session_id"],
        fixture.test.session_configured.thread_id.to_string()
    );
    assert_eq!(
        call["params"]["arguments"]["turn_id"],
        turn_metadata["turn_id"]
    );
    assert_eq!(
        call["params"]["arguments"]["turn_id"],
        response_body["client_metadata"]["turn_id"]
    );
    assert_eq!(
        json!({
            "session_id": turn_metadata["session_id"],
            "thread_id": turn_metadata["thread_id"],
            "model": turn_metadata["model"],
        }),
        json!({
            "session_id": call["params"]["arguments"]["session_id"],
            "thread_id": fixture.test.session_configured.thread_id.to_string(),
            "model": response_body["model"],
        })
    );
    assert_eq!(
        call["params"]["_meta"]["threadId"],
        fixture.test.session_configured.thread_id.to_string()
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn executor_interrupt_hook_runs_after_attachment() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let fixture = executor_hook_fixture(vec![
        completed_turn_response("turn").set_delay(Duration::from_secs(60)),
    ])
    .await?;
    fixture.attach().await?;
    fixture
        .test
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "interrupt this turn".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;

    // The model request confirms discovery was saved, without requiring a target shell.
    tokio::time::timeout(Duration::from_secs(10), async {
        while fixture.responses.requests().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .context("interrupted turn should reach the model request")?;
    fixture.test.codex.submit(Op::Interrupt).await?;
    wait_for_event(&fixture.test.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;
    fixture.wait_for_hook_call().await?;

    let calls = fixture.calls().await?;
    assert_eq!(calls.len(), 1);
    let call = &calls[0]["params"];
    let response = fixture.responses.single_request().body_json();
    let turn_id = &response["client_metadata"]["turn_id"];
    assert_eq!(call["name"], "turn_ended");
    assert_eq!(
        call["arguments"],
        json!({
            "hook_event_name": "Interrupt",
            "session_id": fixture.test.session_configured.thread_id.to_string(),
            "turn_id": turn_id,
        })
    );
    assert_eq!(call["_meta"]["x-codex-turn-metadata"]["turn_id"], *turn_id);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn executor_interrupt_hook_skips_turn_without_step_context() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_remote!(
        Ok(()),
        "standalone user shell commands require a local environment"
    );

    let fixture = executor_hook_fixture(vec![completed_turn_response("first-turn")]).await?;
    fixture.attach().await?;
    fixture
        .test
        .submit_text_turn("populate executor discovery")
        .await?;
    fixture.wait_for_hook_call().await?;

    // Standalone shell turns have no model step, despite the previous turn's discovery.
    fixture
        .test
        .codex
        .submit(Op::RunUserShellCommand {
            command: "sleep 60".to_string(),
            timeout_ms: None,
        })
        .await?;
    fixture.interrupt_running_command().await?;

    assert!(
        tokio::time::timeout(Duration::from_secs(1), fixture.hook_called.notified())
            .await
            .is_err(),
        "executor hooks must not reuse a previous turn's step context",
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn executor_stop_hook_stops_after_disconnection() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let fixture = executor_stop_hook_fixture().await?;
    let selection = fixture.attach().await?;
    fixture
        .test
        .submit_text_turn("before the executor disconnects")
        .await?;
    fixture.wait_for_hook_call().await?;

    fixture
        .test
        .codex
        .environment_failed(&selection, "executor disconnected".to_string())
        .await?;
    fixture
        .test
        .submit_text_turn("after the executor disconnects")
        .await?;

    assert_eq!(fixture.calls().await?.len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn executor_stop_hook_rejects_mismatched_environment() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let fixture = executor_hook_fixture(
        ["first-turn", "switch-environments", "second-turn"]
            .map(completed_turn_response)
            .to_vec(),
    )
    .await?;
    let selection = fixture.attach().await?;
    fixture
        .test
        .submit_text_turn("before the executor environment changes")
        .await?;
    fixture.wait_for_hook_call().await?;

    let (executor_url, executor) =
        if let Some(executor_url) = fixture.test.executor_environment().exec_server_url() {
            (executor_url.to_string(), None)
        } else {
            let listener = TcpListener::bind("127.0.0.1:0").await?;
            let executor_address = listener.local_addr()?;
            let executor_url = format!("ws://{executor_address}");
            drop(listener);
            let runtime_paths = ExecServerRuntimeOptions::new(
                std::env::current_exe()?,
                /*codex_linux_sandbox_exe*/ None,
            )?;
            let http_client_factory = fixture.test.config.http_client_factory();
            let executor_url_for_server = executor_url.clone();
            let executor = tokio::spawn(async move {
                codex_exec_server::run_main(
                    &executor_url_for_server,
                    runtime_paths,
                    http_client_factory,
                )
                .await
            });
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    if tokio::net::TcpStream::connect(executor_address)
                        .await
                        .is_ok()
                    {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .context("timed out waiting for the mismatched executor to start")?;
            (executor_url, Some(executor))
        };

    let mismatched_environment_id = "another-executor";
    let environments = fixture.test.thread_manager.environment_manager();
    environments.upsert_environment(
        mismatched_environment_id.to_string(),
        executor_url,
        /*connect_timeout*/ None,
    )?;
    environments
        .get_environment(mismatched_environment_id)
        .context("mismatched executor environment should exist")?
        .wait_until_ready()
        .await?;
    let attached_selection = fixture
        .test
        .codex
        .environment_selections()
        .await
        .into_iter()
        .next()
        .context("attached executor environment should remain selected")?;
    submit_thread_settings(
        &fixture.test.codex,
        ThreadSettingsOverrides {
            environments: Some(TurnEnvironmentRequests::new(
                fixture.test.config.cwd.clone(),
                vec![
                    attached_selection.into_request(),
                    TurnEnvironmentRequest {
                        environment_id: mismatched_environment_id.to_string(),
                        config: EnvironmentConfigState::FromThread,
                        ..selection.clone().into_request()
                    },
                ],
            )),
            ..Default::default()
        },
    )
    .await?;
    fixture
        .test
        .submit_text_turn("apply the new executor environment")
        .await?;
    fixture.wait_for_hook_call().await?;

    let current_config = fixture.test.codex.config().await;
    let mut mismatched_config = fixture.test.config.clone();
    let mut node_repl = mismatched_config
        .mcp_servers
        .get()
        .get("node_repl")
        .context("Node REPL MCP server should be configured")?
        .clone();
    node_repl.environment_id = mismatched_environment_id.to_string();
    mismatched_config.mcp_servers.set(
        [(String::from("node_repl"), node_repl)]
            .into_iter()
            .collect(),
    )?;
    let _ = fixture
        .test
        .codex
        .refresh_mcp_config(current_config, mismatched_config)
        .await;
    fixture
        .test
        .codex
        .call_mcp_tool(
            "node_repl",
            "js",
            Some(json!({"code": "1 + 1"})),
            /*meta*/ None,
        )
        .await?;
    assert_eq!(
        fixture
            .test
            .codex
            .inspect_selected_capability_roots()
            .ready_roots
            .len(),
        1
    );
    fixture
        .test
        .submit_text_turn("when Node REPL belongs to a different executor")
        .await?;
    if let Some(executor) = executor {
        executor.abort();
    }

    assert_eq!(
        fixture
            .calls()
            .await?
            .iter()
            .filter(|call| call["params"]["name"] == "turn_ended")
            .count(),
        2
    );

    Ok(())
}

#[test_case("Stop", "", "", 1; "enabled")]
#[test_case("Interrupt", "", "", 1; "interrupt_enabled")]
#[test_case("SubagentStop", "", "", 1; "subagent_enabled")]
#[test_case(
    "Stop",
    "[apps.connector_openai_browser]\nenabled = false\n",
    "",
    0;
    "user_disabled"
)]
#[test_case(
    "Stop",
    "[apps.connector_openai_browser]\nenabled = true\n",
    "[apps.connector_openai_browser]\nenabled = false\n",
    0;
    "managed_disabled"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn executor_browser_and_computer_use_cleanup_hooks_use_separate_mcp_routes(
    hook_event: &'static str,
    user_config: &'static str,
    requirements: &'static str,
    expected_browser_calls: usize,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let apps_server = start_mock_server().await;
    let apps = AppsTestServer::mount(&apps_server).await?;
    let routing = json!({
        "resource_uri": "/connector_openai_browser/browser-link/turn_ended",
        "contains_mcp_source": true,
    });
    let listed_routing = routing.clone();
    Mock::given(method("POST"))
        .and(path("/api/codex/ps/mcp"))
        .and(body_partial_json(json!({ "method": "tools/list" })))
        .respond_with(move |request: &Request| {
            let request: Value = serde_json::from_slice(&request.body).expect("valid Apps request");
            ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "result": { "tools": [{
                    "name": "browser.turn_ended",
                    "inputSchema": { "type": "object" },
                    "_meta": {
                        "connector_id": "connector_openai_browser",
                        "connector_name": "Browser",
                        "ui": { "visibility": ["app"] },
                        "_codex_apps": listed_routing,
                    },
                }] },
            }))
        })
        .with_priority(1)
        .mount(&apps_server)
        .await;

    let plugins = [
        (
            "browser@openai-curated-remote",
            json!({
                "name": "browser",
                "hooks": { "hooks": {
                    "Stop": [{ "hooks": [{
                        "type": "mcp_tool",
                        "server": "codex_apps",
                        "tool": "browser.turn_ended",
                        "input": {},
                    }] }],
                    "Interrupt": [{ "hooks": [{
                        "type": "mcp_tool",
                        "server": "codex_apps",
                        "tool": "browser.turn_ended",
                        "input": {},
                    }] }],
                    "SubagentStop": [{ "hooks": [{
                        "type": "mcp_tool",
                        "server": "codex_apps",
                        "tool": "browser.turn_ended",
                        "input": {},
                    }] }],
                } },
            }),
        ),
        ("computer-use@openai-bundled", computer_use_hook_manifest()),
    ];
    let mut fixture = executor_plugin_hook_fixture(
        apps_enabled_builder(apps.chatgpt_base_url)
            .with_pre_build_hook(move |home| {
                std::fs::write(home.join("config.toml"), user_config)
                    .expect("write Browser app configuration");
            })
            .with_cloud_config_bundle(
                CloudConfigBundleFixture::loader_with_enterprise_requirement(requirements),
            ),
        &plugins,
        vec![if hook_event == "Interrupt" {
            completed_turn_response("browser-turn").set_delay(Duration::from_secs(60))
        } else {
            completed_turn_response("browser-turn")
        }],
    )
    .await?;
    if hook_event == "SubagentStop" {
        let child = fixture
            .test
            .thread_manager
            .start_thread(StartThreadOptions {
                session_source: Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id: fixture.test.session_configured.thread_id,
                    depth: 1,
                    agent_path: None,
                    agent_nickname: None,
                    agent_role: None,
                })),
                thread_source: Some(ThreadSource::Subagent),
                ..fixture.test.start_thread_options().await
            })
            .await?;
        fixture.test.codex = child.thread;
        fixture.test.session_configured = child.session_configured;
    }
    fixture.attach().await?;
    if hook_event == "SubagentStop" {
        // Cached subagent MCP servers start on first use; cleanup must not start them.
        fixture
            .test
            .codex
            .call_mcp_tool(
                "node_repl",
                "js",
                Some(json!({ "code": "1 + 1" })),
                /*meta*/ None,
            )
            .await?;
    }
    if hook_event == "Interrupt" {
        fixture
            .test
            .codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "interrupt browsing".to_string(),
                text_elements: Vec::new(),
            }]))
            .await?;
        tokio::time::timeout(Duration::from_secs(10), async {
            while fixture.responses.requests().is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .context("interrupted turn should reach the model request")?;
        fixture.test.codex.submit(Op::Interrupt).await?;
        wait_for_event(&fixture.test.codex, |event| {
            matches!(event, EventMsg::TurnAborted(_))
        })
        .await;
    } else {
        fixture.test.submit_text_turn("finish browsing").await?;
    }
    fixture.wait_for_hook_call().await?;
    let node_calls = fixture.calls().await?;
    let expected_node_tools = if hook_event == "SubagentStop" {
        vec!["js", "turn_ended"]
    } else {
        vec!["turn_ended"]
    };
    assert_eq!(
        node_calls
            .iter()
            .map(|call| call["params"]["name"].as_str().expect("Node tool name"))
            .collect::<Vec<_>>(),
        expected_node_tools
    );
    let node_params = &node_calls.last().context("Node cleanup call")?["params"];
    assert_eq!(node_params["arguments"]["hook_event_name"], hook_event);
    assert!(node_params["_meta"].get("_codex_apps").is_none());

    // Executor cleanup runs in the background, so observe a no-call window after Node runs.
    let browser_timeout = if expected_browser_calls == 0 {
        Duration::from_secs(1)
    } else {
        Duration::from_secs(10)
    };
    let calls = tokio::time::timeout(browser_timeout, async {
        loop {
            let calls = recorded_apps_tool_calls(&apps_server).await;
            if !calls.is_empty() {
                break calls;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_default();
    assert_eq!(calls.len(), expected_browser_calls);
    if calls.is_empty() {
        return Ok(());
    }
    let browser_params = &calls[0]["params"];
    let response_body = fixture.responses.single_request().body_json();
    let thread_id = fixture.test.session_configured.thread_id.to_string();
    assert_eq!(browser_params["name"], "browser.turn_ended");
    assert_eq!(browser_params["arguments"], json!({}));
    assert_eq!(browser_params["_meta"]["_codex_apps"], routing);
    let turn_metadata = &browser_params["_meta"]["x-codex-turn-metadata"];
    assert_eq!(
        node_params["_meta"]["x-codex-turn-metadata"],
        *turn_metadata
    );
    assert_eq!(turn_metadata["thread_id"], thread_id);
    if hook_event == "SubagentStop" {
        assert_eq!(turn_metadata["thread_source"], "subagent");
    }
    assert_eq!(
        turn_metadata["turn_id"],
        response_body["client_metadata"]["turn_id"]
    );
    Ok(())
}

fn completed_turn_response(id: &str) -> ResponseTemplate {
    sse_response(sse(vec![
        ev_response_created(id),
        ev_assistant_message(id, "done"),
        ev_completed(id),
    ]))
}

async fn executor_stop_hook_fixture() -> Result<ExecutorHookFixture> {
    executor_hook_fixture(
        ["first-turn", "second-turn"]
            .map(completed_turn_response)
            .to_vec(),
    )
    .await
}

async fn executor_hook_fixture(responses: Vec<ResponseTemplate>) -> Result<ExecutorHookFixture> {
    executor_plugin_hook_fixture(
        test_codex(),
        &[("computer-use@openai-bundled", computer_use_hook_manifest())],
        responses,
    )
    .await
}

fn computer_use_hook_manifest() -> Value {
    json!({
        "name": "computer-use",
        // Separate entries must all survive registration in the same environment.
        "hooks": [
            { "hooks": { "Interrupt": [{ "hooks": [{
                "type": "mcp_tool",
                "server": "node_repl",
                "tool": "turn_ended",
                "input": {
                    "hook_event_name": "${hook_event_name}",
                    "session_id": "${session_id}",
                    "turn_id": "${turn_id}",
                },
            }] }] } },
            { "hooks": { "Stop": [{ "hooks": [{
                "type": "mcp_tool",
                "server": "node_repl",
                "tool": "turn_ended",
                "input": {
                    "hook_event_name": "${hook_event_name}",
                    "session_id": "${session_id}",
                    "turn_id": "${turn_id}",
                },
            }] }] } },
            { "hooks": { "SubagentStop": [{ "hooks": [{
                "type": "mcp_tool",
                "server": "node_repl",
                "tool": "turn_ended",
                "input": {
                    "hook_event_name": "${hook_event_name}",
                    "session_id": "${session_id}",
                    "turn_id": "${turn_id}",
                },
            }] }] } },
        ],
    })
}

async fn executor_plugin_hook_fixture(
    builder: TestCodexBuilder,
    plugins: &[(&'static str, Value)],
    responses: Vec<ResponseTemplate>,
) -> Result<ExecutorHookFixture> {
    let server = start_mock_server().await;
    let hook_called = Arc::new(Notify::new());
    let hook_called_for_server = Arc::clone(&hook_called);
    Mock::given(method("POST"))
        .and(path("/node-repl"))
        .respond_with(move |request: &Request| {
            let request: Value =
                serde_json::from_slice(&request.body).expect("valid Node REPL JSON-RPC request");
            let result = match request["method"].as_str() {
                Some("initialize") => json!({
                    "protocolVersion": request["params"]["protocolVersion"],
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "node_repl", "version": "1.0.0" },
                }),
                Some("notifications/initialized") => return ResponseTemplate::new(202),
                Some("tools/list") => json!({ "tools": [
                    { "name": "js", "inputSchema": { "type": "object" } },
                    { "name": "turn_ended", "inputSchema": { "type": "object" } },
                ] }),
                Some("tools/call") => {
                    if request["params"]["name"] == "turn_ended" {
                        hook_called_for_server.notify_one();
                    }
                    json!({ "content": [{ "type": "text", "text": "ok" }] })
                }
                method => panic!("unexpected Node REPL request: {method:?}"),
            };
            ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "result": result,
            }))
        })
        .mount(&server)
        .await;

    let node_repl_url = format!("{}/node-repl", server.uri());
    let mut builder = builder.with_config(move |config| {
        config
            .features
            .enable(Feature::ExecutorCapabilityDiscovery)
            .expect("enable executor capability discovery");
        config
            .features
            .disable(Feature::CodexHooks)
            .expect("disable ordinary hooks");
        let node_repl: McpServerConfig = serde_json::from_value(json!({
            "url": node_repl_url,
            "environment_id": remote_aware_environment_id(),
        }))
        .expect("valid Node REPL MCP server configuration");
        config
            .mcp_servers
            .set(
                [(String::from("node_repl"), node_repl)]
                    .into_iter()
                    .collect(),
            )
            .expect("configure Node REPL MCP server");
    });
    let test = builder.build_with_auto_env(&server).await?;
    wait_for_mcp_server(&test.codex, "node_repl").await?;

    let mut plugin_roots = Vec::new();
    let filesystem = test.fs();
    for (plugin_id, manifest) in plugins {
        let plugin_root =
            test.workspace_path_uri(manifest["name"].as_str().context("plugin name")?)?;
        let plugin_directory = plugin_root.join(".codex-plugin")?;
        let manifest_path = plugin_directory.join("plugin.json")?;
        filesystem
            .create_directory(
                &plugin_directory,
                CreateDirectoryOptions {
                    recursive: true,
                    follow_symlinks: true,
                },
                /*sandbox*/ None,
            )
            .await?;
        filesystem
            .write_file(
                &manifest_path,
                serde_json::to_vec(manifest)?,
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
        plugin_roots.push((*plugin_id, plugin_root));
    }

    let responses = mount_response_sequence(&server, responses).await;

    Ok(ExecutorHookFixture {
        server,
        test,
        responses,
        hook_called,
        plugin_roots,
    })
}

struct ExecutorHookFixture {
    server: MockServer,
    test: TestCodex,
    responses: ResponseMock,
    hook_called: Arc<Notify>,
    plugin_roots: Vec<(&'static str, PathUri)>,
}

impl ExecutorHookFixture {
    async fn attach(&self) -> Result<TurnEnvironmentSelection> {
        let selection = self
            .test
            .codex
            .environment_selections()
            .await
            .into_iter()
            .next()
            .context("thread should select its executor environment")?;
        self.test
            .codex
            .environment_ready(
                &selection,
                EnvironmentConfig {
                    environment_hooks: Default::default(),
                    allow_login_shell: false,
                    workspace_roots: selection.workspace_roots.clone(),
                    permission_profile: PermissionProfileSnapshot::legacy(
                        self.test.config.permissions.permission_profile().clone(),
                    ),
                    shell_environment_policy: Default::default(),
                    windows_sandbox_level: WindowsSandboxLevel::from_config(&self.test.config),
                    windows_sandbox_type: self.test.config.permissions.windows_sandbox_type,
                    use_legacy_landlock: self.test.config.features.use_legacy_landlock(),
                    exec_policy: None,
                    mcp_policy: None,
                    network_policy: None,
                    selected_capability_roots: self
                        .plugin_roots
                        .iter()
                        .map(|(plugin_id, plugin_root)| SelectedCapabilityRoot {
                            id: plugin_id.to_string(),
                            location: CapabilityRootLocation::Environment {
                                environment_id: selection.environment_id.clone(),
                                path: plugin_root.clone(),
                            },
                        })
                        .collect(),
                },
            )
            .await?;

        Ok(selection)
    }

    async fn interrupt_running_command(&self) -> Result<()> {
        wait_for_event(&self.test.codex, |event| {
            matches!(event, EventMsg::ExecCommandBegin(_))
        })
        .await;
        self.test.codex.submit(Op::Interrupt).await?;
        wait_for_event(&self.test.codex, |event| {
            matches!(event, EventMsg::TurnAborted(_))
        })
        .await;
        Ok(())
    }

    async fn wait_for_hook_call(&self) -> Result<()> {
        tokio::time::timeout(Duration::from_secs(10), self.hook_called.notified())
            .await
            .context("attached executor hook should call turn_ended")?;

        Ok(())
    }

    async fn calls(&self) -> Result<Vec<Value>> {
        Ok(self
            .server
            .received_requests()
            .await
            .context("mock server should record requests")?
            .into_iter()
            .filter_map(|request| serde_json::from_slice::<Value>(&request.body).ok())
            .filter(|request| request["method"] == "tools/call")
            .collect())
    }
}
