use anyhow::Context;
use anyhow::Result;
use codex_config::McpServerConfig;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::EnvironmentConfig;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_core::windows_sandbox::WindowsSandboxLevelExt;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecServerRuntimePaths;
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
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::protocol::TurnEnvironmentSelections;
use codex_protocol::user_input::UserInput;
use codex_utils_path_uri::PathUri;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::apps_enabled_builder;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_response_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_remote;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
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

    let fixture = executor_stop_hook_fixture().await?;
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
            let runtime_paths = ExecServerRuntimePaths::new(
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
            environments: Some(TurnEnvironmentSelections::new(
                fixture.test.config.cwd.clone(),
                vec![
                    attached_selection,
                    TurnEnvironmentSelection {
                        environment_id: mismatched_environment_id.to_string(),
                        config: EnvironmentConfigState::FromThread,
                        ..selection.clone()
                    },
                ],
            )),
            ..Default::default()
        },
    )
    .await?;

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
    fixture
        .test
        .codex
        .refresh_mcp_config(mismatched_config)
        .await;
    wait_for_mcp_server(&fixture.test.codex, "node_repl").await?;
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

    assert_eq!(fixture.calls().await?.len(), 1);

    Ok(())
}

#[test_case("Stop", "", "", 1; "enabled")]
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
async fn executor_browser_and_computer_use_stop_hooks_use_separate_mcp_routes(
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
        vec![completed_turn_response("browser-turn")],
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
                environments: Some(fixture.test.codex.environment_selections().await),
                ..StartThreadOptions::new(fixture.test.config.clone())
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
    fixture.test.submit_text_turn("finish browsing").await?;
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
                    allow_login_shell: false,
                    workspace_roots: selection.workspace_roots.clone(),
                    permission_profile: PermissionProfileSnapshot::legacy(
                        self.test.config.permissions.permission_profile().clone(),
                    ),
                    shell_environment_policy: Default::default(),
                    windows_sandbox_level: WindowsSandboxLevel::from_config(&self.test.config),
                    windows_sandbox_private_desktop: self
                        .test
                        .config
                        .permissions
                        .windows_sandbox_private_desktop,
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
