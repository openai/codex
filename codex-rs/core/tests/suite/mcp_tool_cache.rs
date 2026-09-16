use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context;
use codex_config::Constrained;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use codex_core::NewThread;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::RemoveOptions;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_protocol::mcp::McpServerConnectionStatus;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpInvocation;
use codex_protocol::protocol::McpStartupStatus;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use codex_utils_path_uri::PathUri;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_CREATE_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::apps_enabled_builder;
use core_test_support::is_remote_test_environment;
use core_test_support::responses;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::mount_sse_once;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::test_env;
use core_test_support::wait_for_event;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use test_case::test_case;
use tracing::Subscriber;
use tracing::span::Attributes;
use tracing::span::Id;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context as LayerContext;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use super::rmcp_client::remote_aware_environment_id;
use super::rmcp_client::remote_aware_stdio_server_bin;

const SERVER_NAME: &str = "cached_rmcp";
const NAMESPACE: &str = "mcp__cached_rmcp";

struct McpBindingCaptureCounter(Arc<AtomicUsize>);

impl<S: Subscriber> Layer<S> for McpBindingCaptureCounter {
    fn on_new_span(&self, attributes: &Attributes<'_>, _id: &Id, _context: LayerContext<'_, S>) {
        if attributes.metadata().target() == "codex_mcp::connection_manager::tool_catalog"
            && attributes.metadata().name() == "capture_binding_with_metadata"
        {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
}

fn user_turn(prompt: &str) -> TurnInputRequest {
    TurnInputRequest::user_input(vec![UserInput::Text {
        text: prompt.to_string(),
        text_elements: Vec::new(),
    }])
    .with_thread_settings(ThreadSettingsOverrides {
        approval_policy: Some(AskForApproval::Never),
        permission_profile: Some(PermissionProfile::Disabled),
        ..Default::default()
    })
}

fn process_label(pid: &str) -> String {
    format!("rmcp-test-process-{pid}")
}

fn assert_definition(response: &ResponseMock, namespace_description: &str, tool_description: &str) {
    let body = response.single_request().body_json();
    let namespace = body
        .get("tools")
        .and_then(Value::as_array)
        .and_then(|tools| {
            tools
                .iter()
                .find(|tool| tool.get("name").and_then(Value::as_str) == Some(NAMESPACE))
        })
        .expect("request should contain the MCP namespace");
    assert_eq!(
        namespace.get("description").and_then(Value::as_str),
        Some(namespace_description)
    );
    assert_eq!(
        responses::namespace_child_tool(&body, NAMESPACE, "echo")
            .and_then(|tool| tool.get("description"))
            .and_then(Value::as_str),
        Some(tool_description)
    );
}

async fn wait_for_new_pid(
    fs: &dyn ExecutorFileSystem,
    path: &PathUri,
    previous_pid: Option<&str>,
) -> anyhow::Result<String> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(contents) = fs
                .read_file_text(path, Default::default(), /*sandbox*/ None)
                .await
            {
                let pid = contents.trim();
                if !pid.is_empty() && Some(pid) != previous_pid {
                    return pid.to_string();
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("timed out waiting for a new MCP server process")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_calls_stay_bound_to_each_thread() -> anyhow::Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    let responses_server = responses::start_mock_server().await;
    let command = remote_aware_stdio_server_bin()?;
    let environment_id = remote_aware_environment_id();
    let test_env = test_env().await?;
    let make_server = |marker| {
        serde_json::from_value::<McpServerConfig>(json!({
            "command": command,
            "environment_id": environment_id,
            "cwd": test_env.cwd(),
            "env": {
                "MCP_TEST_DYNAMIC_SERVER_METADATA": "1",
                "MCP_TEST_VALUE": marker,
            },
            "enabled_tools": ["echo"],
            "startup_timeout_sec": 10,
        }))
    };
    let first_server = make_server("first-runtime")?;
    let second_server = make_server("second-runtime")?;
    let fixture = test_codex()
        .with_model_info_override("gpt-5.4", |model| model.supports_search_tool = false)
        .with_config(move |config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::Never);
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .expect("first thread should accept disabled permissions");
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(SERVER_NAME.to_string(), first_server);
            config
                .mcp_servers
                .set(servers)
                .expect("first thread should accept its MCP servers");
        })
        .build_with_environment(&responses_server, test_env)
        .await?;

    let mut second_config = fixture.config.clone();
    let mut second_servers = second_config.mcp_servers.get().clone();
    second_servers.insert(SERVER_NAME.to_string(), second_server);
    second_config.mcp_servers.set(second_servers)?;
    let NewThread {
        thread: second_thread,
        ..
    } = fixture
        .thread_manager
        .start_thread(StartThreadOptions::new(second_config))
        .await?;

    wait_for_mcp_server(&fixture.codex, SERVER_NAME).await?;
    wait_for_mcp_server(&second_thread, SERVER_NAME).await?;

    let calls = [
        (&fixture.codex, "first-call", "first-runtime"),
        (&second_thread, "second-call", "second-runtime"),
        (&fixture.codex, "first-again", "first-runtime"),
    ];
    let mut processes = Vec::new();
    for (thread, call_id, marker) in calls {
        let call_response = mount_sse_once(
            &responses_server,
            responses::sse(vec![
                responses::ev_response_created(call_id),
                responses::ev_function_call_with_namespace(
                    call_id,
                    NAMESPACE,
                    "echo",
                    &json!({ "message": call_id }).to_string(),
                ),
                responses::ev_completed(call_id),
            ]),
        )
        .await;
        let completion_response = mount_sse_once(
            &responses_server,
            responses::sse(vec![
                responses::ev_response_created(&format!("{call_id}-done")),
                responses::ev_assistant_message(call_id, "done"),
                responses::ev_completed(&format!("{call_id}-done")),
            ]),
        )
        .await;
        thread
            .start_or_steer_turn(user_turn(&format!("Call the {SERVER_NAME} echo tool.")))
            .await?;
        let EventMsg::McpToolCallEnd(end) = wait_for_event(
            thread,
            |event| matches!(event, EventMsg::McpToolCallEnd(end) if end.call_id == call_id),
        )
        .await
        else {
            unreachable!("event predicate guarantees the requested MCP result");
        };
        assert_eq!(
            end.invocation,
            McpInvocation {
                server: SERVER_NAME.to_string(),
                tool: "echo".to_string(),
                arguments: Some(json!({ "message": call_id })),
            }
        );
        let content = end
            .result
            .expect("thread-local MCP call should succeed")
            .structured_content
            .expect("echo should return structured content");
        let process = content
            .get("echo")
            .and_then(Value::as_str)
            .expect("echo should identify its server process")
            .to_string();
        assert!(process.starts_with("rmcp-test-process-"));
        assert_eq!(content, json!({ "echo": process, "env": marker }));
        wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        let request = call_response.single_request();
        assert!(request.tool_by_name(NAMESPACE, "echo").is_some());
        let completion_request = completion_response.single_request();
        assert_eq!(
            request.body_json()["tools"],
            completion_request.body_json()["tools"],
            "MCP tool schemas must remain unchanged across a same-turn continuation"
        );
        let output = completion_request
            .function_call_output_text(call_id)
            .expect("MCP result should be returned to the model");
        assert!(output.contains(&process));
        assert!(output.contains(marker));
        processes.push(process);
    }

    assert_ne!(processes[0], processes[1]);
    assert_eq!(processes[0], processes[2]);

    fixture.codex.shutdown_and_wait().await?;
    second_thread.shutdown_and_wait().await?;
    responses_server.verify().await;
    Ok(())
}

#[tokio::test]
async fn apps_cache_filled_during_binding_capture_reaches_the_model() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    struct ChildAppsEndpoint(String);

    impl McpServerContributor<Config> for ChildAppsEndpoint {
        fn id(&self) -> &'static str {
            "child_apps_cache_test"
        }

        fn contribute<'a>(
            &'a self,
            context: McpServerContributionContext<'a, Config>,
        ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
            Box::pin(async move {
                if !matches!(context.session_source(), Some(SessionSource::SubAgent(_))) {
                    return Vec::new();
                }
                vec![McpServerContribution::HostedApps {
                    config: Box::new(
                        serde_json::from_value(json!({ "url": self.0 }))
                            .expect("child Apps MCP config"),
                    ),
                    protocol_mode: None,
                }]
            })
        }
    }

    tokio::time::timeout(Duration::from_secs(30), async {
        let server = responses::start_mock_server().await;
        let tools_available = Arc::new(AtomicBool::new(false));
        let apps =
            AppsTestServer::mount_with_tools_available_when(&server, Arc::clone(&tools_available))
                .await?;
        let waiting_server = responses::start_mock_server().await;
        let (waiting, waiting_startup) =
            AppsTestServer::mount_with_startup_control(&waiting_server).await?;
        let pending_server = responses::start_mock_server().await;
        let (pending_apps, pending_startup) =
            AppsTestServer::mount_with_startup_control(&pending_server).await?;
        // Gate only the child's MCP endpoint, leaving backend directory requests unblocked.
        let mut extensions = ExtensionRegistryBuilder::new();
        extensions.mcp_server_contributor(Arc::new(ChildAppsEndpoint(format!(
            "{}/api/codex/ps/mcp",
            pending_apps.chatgpt_base_url
        ))));
        let test = apps_enabled_builder(apps.chatgpt_base_url)
            .with_model_info_override("gpt-5.5", |model| model.supports_search_tool = false)
            .with_extensions(Arc::new(extensions.build()))
            .with_config(move |config| {
                // Keep the gated HTTP request on the app host. Wine's executor serializes RPCs,
                // so blocking it here would also block the peer startup that releases this gate.
                config
                    .mcp_servers
                    .set(std::collections::HashMap::from([(
                        SERVER_NAME.to_string(),
                        serde_json::from_value(json!({
                            "url": format!("{}/api/codex/ps/mcp", waiting.chatgpt_base_url),
                            "http_headers": { "Authorization": "Bearer cache-test-token" },
                            "enabled_tools": ["calendar_list_events"],
                        }))
                        .expect("cacheable MCP config"),
                    )]))
                    .expect("test MCP config");
            })
            .build_with_auto_env(&server)
            .await?;
        // Startup emits one summary for both servers, not one event per server.
        let EventMsg::McpStartupComplete(startup) = wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::McpStartupComplete(_))
        })
        .await
        else {
            unreachable!("event predicate guarantees the startup summary");
        };
        assert_eq!(
            startup
                .ready
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from([
                CODEX_APPS_MCP_SERVER_NAME.to_string(),
                SERVER_NAME.to_string(),
            ])
        );

        let release_apps = pending_startup.hold_next_successful_initialize();
        let release_waiting = waiting_startup.hold_next_successful_initialize();
        let NewThread { thread: child, .. } = test
            .thread_manager
            .start_thread(StartThreadOptions {
                session_source: Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id: test.session_configured.thread_id,
                    depth: 1,
                    agent_path: None,
                    agent_nickname: None,
                    agent_role: None,
                })),
                ..StartThreadOptions::new(test.config.clone())
            })
            .await?;
        assert_eq!(waiting_startup.initialize_attempts(), 1);
        let response = mount_sse_once(
            &server,
            responses::sse(vec![
                responses::ev_response_created("cache-refresh"),
                responses::ev_assistant_message("done", "done"),
                responses::ev_completed("cache-refresh"),
            ]),
        )
        .await;
        child
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Mention {
                name: SERVER_NAME.to_string(),
                path: format!("mcp://{SERVER_NAME}"),
            }]))
            .await?;
        wait_for_event(&child, |event| {
            matches!(event, EventMsg::McpStartupUpdate(update)
            if update.server == SERVER_NAME && matches!(update.status, McpStartupStatus::Starting))
        })
        .await;

        // A peer fills the initially empty shared Apps cache while capture waits for the named server.
        tools_available.store(true, Ordering::SeqCst);
        let mut peer_config = test.config.clone();
        peer_config
            .mcp_servers
            .set(std::collections::HashMap::new())?;
        let NewThread { thread: peer, .. } = test
            .thread_manager
            .start_thread(StartThreadOptions::new(peer_config))
            .await?;
        wait_for_mcp_server(&peer, CODEX_APPS_MCP_SERVER_NAME).await?;
        release_waiting.send(())?;
        wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        // The child's own Apps client must still be pending when its request reaches inference.
        release_apps.send(())?;
        let body = response.single_request().body_json();
        assert!(
            responses::namespace_child_tool(
                &body,
                SEARCH_CALENDAR_NAMESPACE,
                SEARCH_CALENDAR_CREATE_TOOL,
            )
            .is_some(),
            "the first model request must include the peer's Apps tools: {body}"
        );

        child.shutdown_and_wait().await?;
        peer.shutdown_and_wait().await?;
        Ok(())
    })
    .await
    .context("timed out exercising an Apps cache fill during binding capture")?
}

#[test_case(false, false, 1; "optional server uses cache")]
#[test_case(true, false, 1; "required server uses cache")]
#[test_case(false, true, 2; "headers helper bypasses cache")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cached_http_mcp_starts_lazily_for_subagents(
    required: bool,
    with_headers_helper: bool,
    expected_startup_attempts: usize,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    if with_headers_helper && is_remote_test_environment() {
        return Ok(());
    }

    let responses_server = responses::start_mock_server().await;
    let (http_server, startup_control) =
        AppsTestServer::mount_with_startup_control(&responses_server).await?;
    let server_url = format!("{}/api/codex/ps/mcp", http_server.chatgpt_base_url);
    let fixture = test_codex()
        .with_model_info_override("gpt-5.4", |model| model.supports_search_tool = false)
        .with_config(move |config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::Never);
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .expect("test config should allow disabled permissions");
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                SERVER_NAME.to_string(),
                serde_json::from_value(json!({
                    "url": server_url,
                    "http_headers": { "Authorization": "Bearer cached-http-test-token" },
                    "enabled_tools": ["calendar_create_event"],
                    "required": required,
                    "startup_timeout_sec": 10,
                }))
                .expect("HTTP MCP server configuration"),
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test MCP server configuration");
        })
        .build_with_auto_env(&responses_server)
        .await?;
    wait_for_mcp_server(&fixture.codex, SERVER_NAME).await?;
    assert_eq!(startup_control.initialize_attempts(), 1);

    let mut subagent_config = fixture.config.clone();
    if with_headers_helper {
        let mut servers = subagent_config.mcp_servers.get().clone();
        let server = servers.get_mut(SERVER_NAME).expect("cached HTTP server");
        let McpServerTransportConfig::StreamableHttp {
            http_headers_helper,
            ..
        } = &mut server.transport
        else {
            unreachable!("expected HTTP transport");
        };
        *http_headers_helper = Some(if cfg!(windows) {
            r#"echo {"X-Cache-Test":"helper"}"#.to_string()
        } else {
            r#"printf '{"X-Cache-Test":"helper"}'"#.to_string()
        });
        subagent_config.mcp_servers.set(servers)?;
    }
    let NewThread {
        thread: subagent, ..
    } = fixture
        .thread_manager
        .start_thread(StartThreadOptions {
            session_source: Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: fixture.session_configured.thread_id,
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            })),
            ..StartThreadOptions::new(subagent_config)
        })
        .await?;
    if with_headers_helper {
        wait_for_mcp_server(&subagent, SERVER_NAME).await?;
    }
    assert_eq!(
        startup_control.initialize_attempts(),
        expected_startup_attempts
    );

    let call_id = "http-call";
    let call_response = mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created(call_id),
            responses::ev_function_call_with_namespace(
                call_id,
                NAMESPACE,
                "calendar_create_event",
                r#"{"title":"cached","starts_at":"2026-01-01T00:00:00Z"}"#,
            ),
            responses::ev_completed(call_id),
        ]),
    )
    .await;
    let completion_response = mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_assistant_message("http-call-message", "done"),
            responses::ev_completed("http-call-done"),
        ]),
    )
    .await;
    subagent
        .start_or_steer_turn(user_turn("Call the cached HTTP tool."))
        .await?;
    wait_for_event(&subagent, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let request = call_response.single_request();
    assert!(
        request
            .tool_by_name(NAMESPACE, "calendar_create_event")
            .is_some()
    );
    let output = completion_response.function_call_output_text(call_id);
    assert!(output.is_some());
    assert_eq!(startup_control.initialize_attempts(), 2);

    fixture.codex.shutdown_and_wait().await?;
    subagent.shutdown_and_wait().await?;
    responses_server.verify().await;
    Ok(())
}

// Keep spawned tasks on the thread with the scoped tracing subscriber.
#[tokio::test(flavor = "current_thread")]
async fn cached_mcp_startup_is_eager_for_root_and_lazy_for_subagents() -> anyhow::Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    let binding_captures = Arc::new(AtomicUsize::new(0));
    let _tracing = tracing_subscriber::registry()
        .with(McpBindingCaptureCounter(Arc::clone(&binding_captures)))
        .set_default();

    let responses_server = responses::start_mock_server().await;
    let command = remote_aware_stdio_server_bin()?;
    let environment_id = remote_aware_environment_id();
    let fixture = test_codex()
        .with_model_info_override("gpt-5.4", |model| model.supports_search_tool = false)
        .with_config(move |config| {
            config.update_plan_enabled = true;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::Never);
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .expect("test config should allow disabled permissions");
            let app_only_cwd_marker_file = config.cwd.join("cwd-app-only");
            let barrier_file = config.cwd.join("allow-initialize");
            let pid_file = config.cwd.join("mcp.pid");
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                SERVER_NAME.to_string(),
                serde_json::from_value(json!({
                    "command": command,
                    "environment_id": environment_id,
                    "cwd": config.cwd,
                    "env": {
                        "MCP_TEST_APP_ONLY_CWD_MARKER_FILE": app_only_cwd_marker_file,
                        "MCP_TEST_INITIALIZE_BARRIER_FILE": barrier_file,
                        "MCP_TEST_DYNAMIC_SERVER_METADATA": "1",
                        "MCP_TEST_PID_FILE": pid_file,
                    },
                    "enabled_tools": ["cwd", "echo"],
                    "startup_timeout_sec": 10,
                }))
                .expect("test MCP server configuration"),
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test MCP server configuration");
        })
        .build_with_auto_env(&responses_server)
        .await?;
    let fs = fixture.fs();
    let app_only_cwd_marker_file =
        PathUri::from_host_native_path(fixture.config.cwd.join("cwd-app-only"))?;
    let barrier_file = PathUri::from_host_native_path(fixture.config.cwd.join("allow-initialize"))?;
    let pid_file = PathUri::from_host_native_path(fixture.config.cwd.join("mcp.pid"))?;

    let cold_response = mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("cold"),
            responses::ev_assistant_message("cold-message", "done"),
            responses::ev_completed("cold"),
        ]),
    )
    .await;
    fixture
        .codex
        .start_or_steer_turn(user_turn("use the echo tool"))
        .await?;
    let first_pid = wait_for_new_pid(fs.as_ref(), &pid_file, /*previous_pid*/ None).await?;
    fs.write_file(
        &barrier_file,
        b"ready".to_vec(),
        Default::default(),
        /*sandbox*/ None,
    )
    .await?;
    wait_for_event(&fixture.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let first_process = process_label(&first_pid);
    assert_definition(
        &cold_response,
        &format!("Use the tools from {first_process}."),
        &format!("Echo from {first_process}."),
    );

    let NewThread {
        thread: eager_thread,
        ..
    } = fixture
        .thread_manager
        .start_thread(StartThreadOptions::new(fixture.config.clone()))
        .await?;
    let eager_pid = wait_for_new_pid(fs.as_ref(), &pid_file, Some(&first_pid)).await?;
    wait_for_mcp_server(&eager_thread, SERVER_NAME).await?;
    eager_thread.shutdown_and_wait().await?;
    let cached_process = process_label(&eager_pid);

    fs.remove(
        &barrier_file,
        RemoveOptions {
            recursive: false,
            force: false,
            follow_symlinks: true,
        },
        /*sandbox*/ None,
    )
    .await?;
    fs.write_file(
        &app_only_cwd_marker_file,
        b"app-only".to_vec(),
        Default::default(),
        /*sandbox*/ None,
    )
    .await?;
    let NewThread {
        thread: second_thread,
        ..
    } = fixture
        .thread_manager
        .start_thread(StartThreadOptions {
            session_source: Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: fixture.session_configured.thread_id,
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            })),
            ..StartThreadOptions::new(fixture.config.clone())
        })
        .await?;
    second_thread.submit(Op::Interrupt).await?;

    binding_captures.store(0, Ordering::SeqCst);
    let unused_response = mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("unused"),
            responses::ev_function_call(
                "unused-plan",
                "update_plan",
                r#"{"plan":[{"step":"Work without MCP tools","status":"in_progress"}]}"#,
            ),
            responses::ev_completed("unused"),
        ]),
    )
    .await;
    let unused_done_response = mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("unused-done"),
            responses::ev_assistant_message("unused-message", "done"),
            responses::ev_completed("unused-done"),
        ]),
    )
    .await;
    second_thread
        .start_or_steer_turn(user_turn("Do not call any MCP tools."))
        .await?;
    let mut reported_ready_before_startup = false;
    wait_for_event(&second_thread, |event| {
        if let EventMsg::McpStartupUpdate(update) = event
            && update.server == SERVER_NAME
            && matches!(update.status, McpStartupStatus::Ready)
        {
            reported_ready_before_startup = true;
        }
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert!(
        !reported_ready_before_startup,
        "a dormant MCP server must not be reported as ready"
    );
    for response in [&unused_response, &unused_done_response] {
        assert_definition(
            response,
            &format!("Use the tools from {cached_process}."),
            &format!("Echo from {cached_process}."),
        );
    }
    assert_eq!(
        binding_captures.load(Ordering::SeqCst),
        1,
        "both model steps must share one binding while the cached server stays dormant"
    );
    let (mcp_config, _) = second_thread.current_mcp_config_and_runtime_context().await;
    assert_eq!(
        second_thread
            .mcp_connection_statuses(&mcp_config)
            .await
            .get(SERVER_NAME),
        Some(&McpServerConnectionStatus::NotStarted),
    );
    assert_eq!(
        fs.read_file_text(&pid_file, Default::default(), /*sandbox*/ None)
            .await?
            .trim(),
        eager_pid,
        "cached tool definitions should not start an unused subagent-owned MCP process"
    );

    let app_only_call_id = "cached-app-only-call";
    let unrelated_call_id = "cached-unrelated-plan";
    let cached_response = mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("cached-call"),
            responses::ev_function_call_with_namespace(
                "cached-call",
                NAMESPACE,
                "echo",
                r#"{"message":"hello"}"#,
            ),
            responses::ev_function_call_with_namespace(app_only_call_id, NAMESPACE, "cwd", "{}"),
            responses::ev_function_call(
                unrelated_call_id,
                "update_plan",
                r#"{"plan":[{"step":"Continue while MCP starts","status":"in_progress"}]}"#,
            ),
            responses::ev_completed("cached-call"),
        ]),
    )
    .await;
    let cached_done_response = mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("cached-done"),
            responses::ev_assistant_message("cached-message", "done"),
            responses::ev_completed("cached-done"),
        ]),
    )
    .await;
    let (unrelated_finished_tx, unrelated_finished_rx) = tokio::sync::oneshot::channel();
    let second_for_turn = Arc::clone(&second_thread);
    let cached_turn = tokio::spawn(async move {
        second_for_turn
            .start_or_steer_turn(user_turn("call the echo and cwd tools"))
            .await?;
        let mut unrelated_finished_tx = Some(unrelated_finished_tx);
        let mut saw_starting = false;
        let mut saw_ready = false;
        let end = wait_for_event(&second_for_turn, |event| {
            if matches!(event, EventMsg::PlanUpdate(_))
                && let Some(sender) = unrelated_finished_tx.take()
            {
                let _ = sender.send(());
            }
            if let EventMsg::McpStartupUpdate(update) = event
                && update.server == SERVER_NAME
            {
                saw_starting |= matches!(update.status, McpStartupStatus::Starting);
                saw_ready |= matches!(update.status, McpStartupStatus::Ready);
            }
            matches!(
                event,
                EventMsg::McpToolCallEnd(end) if end.call_id == "cached-call"
            )
        })
        .await;
        assert!(
            saw_starting,
            "deferred startup should emit its starting status"
        );
        assert!(saw_ready, "deferred startup should emit its ready status");
        let EventMsg::McpToolCallEnd(end) = end else {
            unreachable!("event predicate guarantees an MCP tool result");
        };
        let called_process = end
            .result
            .expect("echo call should succeed")
            .structured_content
            .and_then(|content| content.get("echo").cloned())
            .and_then(|echo| echo.as_str().map(ToString::to_string))
            .expect("echo result should identify its live server process");
        wait_for_event(&second_for_turn, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        anyhow::Ok(called_process)
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while cached_response.requests().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("cached MCP definitions should reach inference before initialization")?;
    assert_definition(
        &cached_response,
        &format!("Use the tools from {cached_process}."),
        &format!("Echo from {cached_process}."),
    );
    let second_pid = wait_for_new_pid(fs.as_ref(), &pid_file, Some(&eager_pid)).await?;
    let second_process = process_label(&second_pid);
    tokio::time::timeout(Duration::from_secs(2), unrelated_finished_rx)
        .await
        .context("an unrelated tool should complete while cached MCP startup is pending")?
        .context("the unrelated tool should emit its plan update")?;

    fixture.codex.shutdown_and_wait().await?;
    fs.write_file(
        &barrier_file,
        b"ready".to_vec(),
        Default::default(),
        /*sandbox*/ None,
    )
    .await?;
    let expected_error = format!("MCP tool `{SERVER_NAME}/cwd` is not available to the model");
    assert_eq!(cached_turn.await??, second_process);
    assert!(
        binding_captures.load(Ordering::SeqCst) > 1,
        "starting the cached server must invalidate the dormant binding"
    );
    assert_definition(
        &cached_done_response,
        &format!("Use the tools from {second_process}."),
        &format!("Echo from {second_process}."),
    );
    let output_item = cached_done_response
        .single_request()
        .function_call_output(app_only_call_id);
    let output = output_item["output"][1]["text"]
        .as_str()
        .expect("app-only tool error should be returned to the model");
    assert!(
        output.contains(&expected_error),
        "model-visible tool output should contain the live visibility error: {output}"
    );
    let output = cached_done_response
        .single_request()
        .function_call_output_text("cached-call")
        .expect("successful tool output should be returned to the model");
    assert!(
        output.contains(&second_process),
        "model-visible tool output should come from the live server: {output}"
    );
    assert_eq!(
        cached_done_response
            .single_request()
            .function_call_output_text(unrelated_call_id)
            .as_deref(),
        Some("Plan updated")
    );

    second_thread.shutdown_and_wait().await?;
    let mut filtered_config = fixture.config.clone();
    let mut filtered_servers = filtered_config.mcp_servers.get().clone();
    filtered_servers
        .get_mut(SERVER_NAME)
        .expect("cached MCP server should remain configured")
        .enabled_tools = Some(vec!["cwd".to_string()]);
    filtered_config.mcp_servers.set(filtered_servers)?;
    let NewThread {
        thread: filtered_thread,
        ..
    } = fixture
        .thread_manager
        .start_thread(StartThreadOptions {
            session_source: Some(SessionSource::SubAgent(SubAgentSource::Other(
                "filtered-cached-startup".to_string(),
            ))),
            ..StartThreadOptions::new(filtered_config)
        })
        .await?;
    let filtered_pid = wait_for_new_pid(fs.as_ref(), &pid_file, Some(&second_pid)).await?;
    wait_for_mcp_server(&filtered_thread, SERVER_NAME).await?;
    filtered_thread.shutdown_and_wait().await?;
    fs.remove(
        &barrier_file,
        RemoveOptions {
            recursive: false,
            force: false,
            follow_symlinks: true,
        },
        /*sandbox*/ None,
    )
    .await?;
    let NewThread {
        thread: interrupted_thread,
        ..
    } = fixture
        .thread_manager
        .start_thread(StartThreadOptions {
            session_source: Some(SessionSource::SubAgent(SubAgentSource::Other(
                "interrupted-cached-startup".to_string(),
            ))),
            ..StartThreadOptions::new(fixture.config.clone())
        })
        .await?;
    mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("interrupted-startup"),
            responses::ev_function_call_with_namespace(
                "interrupted-startup-call",
                NAMESPACE,
                "echo",
                r#"{"message":"interrupted"}"#,
            ),
            responses::ev_completed("interrupted-startup"),
        ]),
    )
    .await;
    interrupted_thread
        .start_or_steer_turn(user_turn("Start the cached MCP tool."))
        .await?;
    let interrupted_pid = wait_for_new_pid(fs.as_ref(), &pid_file, Some(&filtered_pid)).await?;
    wait_for_event(&interrupted_thread, |event| {
        matches!(
            event,
            EventMsg::McpStartupUpdate(update)
                if update.server == SERVER_NAME
                    && matches!(update.status, McpStartupStatus::Starting)
        )
    })
    .await;
    interrupted_thread.submit(Op::Interrupt).await?;
    wait_for_event(&interrupted_thread, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;
    fs.write_file(
        &barrier_file,
        b"ready".to_vec(),
        Default::default(),
        /*sandbox*/ None,
    )
    .await?;
    tokio::time::timeout(
        Duration::from_secs(2),
        wait_for_event(&interrupted_thread, |event| {
            matches!(
                event,
                EventMsg::McpStartupUpdate(update)
                    if update.server == SERVER_NAME
                        && matches!(update.status, McpStartupStatus::Ready)
            )
        }),
    )
    .await
    .context("deferred MCP startup should survive an interrupted first tool call")?;
    mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("retried-startup"),
            responses::ev_function_call_with_namespace(
                "retried-startup-call",
                NAMESPACE,
                "echo",
                r#"{"message":"retried"}"#,
            ),
            responses::ev_completed("retried-startup"),
        ]),
    )
    .await;
    let retry_done = mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("retried-done"),
            responses::ev_assistant_message("retried-message", "done"),
            responses::ev_completed("retried-done"),
        ]),
    )
    .await;
    interrupted_thread
        .start_or_steer_turn(user_turn("Retry the cached MCP tool."))
        .await?;
    wait_for_event(&interrupted_thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let retry_output = retry_done
        .single_request()
        .function_call_output_text("retried-startup-call")
        .expect("the retried MCP tool call should return its output");
    assert!(
        retry_output.contains(&process_label(&interrupted_pid)),
        "the retry should use the server started by the interrupted call"
    );
    interrupted_thread.shutdown_and_wait().await?;
    responses_server.verify().await;
    Ok(())
}
