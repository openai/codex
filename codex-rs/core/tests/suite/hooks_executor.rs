use anyhow::Context;
use anyhow::Result;
use codex_config::McpServerConfig;
use codex_core::EnvironmentConfig;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecServerRuntimePaths;
use codex_features::Feature;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::models::PermissionProfileSnapshot;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::protocol::TurnEnvironmentSelections;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::rmcp_client::remote_aware_environment_id;

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
            let executor_url = format!("ws://{}", listener.local_addr()?);
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
            tokio::task::yield_now().await;
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

async fn executor_stop_hook_fixture() -> Result<ExecutorStopHookFixture> {
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
                Some("tools/list") => json!({ "tools": [{
                    "name": "turn_ended",
                    "inputSchema": { "type": "object" },
                }] }),
                Some("tools/call") => {
                    hook_called_for_server.notify_one();
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
    let mut builder = test_codex().with_config(move |config| {
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

    let plugin_root = test.workspace_path_uri("computer-use")?;
    let plugin_directory = plugin_root.join(".codex-plugin")?;
    let manifest_path = plugin_directory.join("plugin.json")?;
    let filesystem = test.fs();
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
            serde_json::to_vec(&json!({
                "name": "computer-use",
                "hooks": { "hooks": { "Stop": [{ "hooks": [{
                    "type": "mcp_tool",
                    "server": "node_repl",
                    "tool": "turn_ended",
                    "input": {
                        "hook_event_name": "${hook_event_name}",
                        "session_id": "${session_id}",
                        "turn_id": "${turn_id}",
                    },
                }] }] } },
            }))?,
            Default::default(),
            /*sandbox*/ None,
        )
        .await?;

    let responses = mount_sse_sequence(
        &server,
        ["first-turn", "second-turn"]
            .map(|id| {
                sse(vec![
                    ev_response_created(id),
                    ev_assistant_message(id, "done"),
                    ev_completed(id),
                ])
            })
            .to_vec(),
    )
    .await;

    Ok(ExecutorStopHookFixture {
        server,
        test,
        responses,
        hook_called,
    })
}

struct ExecutorStopHookFixture {
    server: MockServer,
    test: TestCodex,
    responses: ResponseMock,
    hook_called: Arc<Notify>,
}

impl ExecutorStopHookFixture {
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
                    permission_profile: PermissionProfileSnapshot::legacy(
                        self.test.config.permissions.permission_profile().clone(),
                    ),
                    shell_environment_policy: Default::default(),
                    exec_policy: None,
                    mcp_policy: None,
                    network_policy: None,
                    selected_capability_roots: vec![SelectedCapabilityRoot {
                        id: "computer-use@openai-bundled".to_string(),
                        location: CapabilityRootLocation::Environment {
                            environment_id: selection.environment_id.clone(),
                            path: self.test.workspace_path_uri("computer-use")?,
                        },
                    }],
                },
            )
            .await?;

        Ok(selection)
    }

    async fn wait_for_hook_call(&self) -> Result<()> {
        tokio::time::timeout(Duration::from_secs(10), self.hook_called.notified())
            .await
            .context("attached executor Stop hook should call turn_ended")?;

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
