use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_mock_responses_config_toml_with_chatgpt_base_url;
use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::CapabilityRootLocation;
use codex_app_server_protocol::EnvironmentAddResponse;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::ListMcpServerStatusResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SelectedCapabilityRoot;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnEnvironmentParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::protocol::PLUGINS_INSTRUCTIONS_OPEN_TAG;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use core_test_support::process::wait_for_pid_file;
use core_test_support::responses;
use core_test_support::responses::ResponsesRequest;
use core_test_support::stdio_server_bin;
use pretty_assertions::assert_eq;
use pretty_assertions::assert_ne;
use serde_json::json;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::Command;
use tokio::time::timeout;

use super::app_list::connector_tool;
use super::app_list::start_apps_server_with_delays;

const READ_TIMEOUT: Duration = Duration::from_secs(20);
const EXECUTOR_ID: &str = "executor-1";
const EXECUTOR_ENV_NAME: &str = "MCP_EXECUTOR_MARKER";
const EXECUTOR_ENV_VALUE: &str = "executor-only";
const PLUGIN_ID: &str = "executor-demo@1";
const PLUGIN_DISPLAY_NAME: &str = "Executor Demo";
const SKILL_NAME: &str = "executor-demo:deploy";
const SKILL_DESCRIPTION: &str = "Deploy through the selected executor.";
const SKILL_BODY_MARKER: &str = "SELECTED_EXECUTOR_SKILL_BODY";
const LOCAL_SKILL_BODY_MARKER: &str = "COLLIDING_LOCAL_SKILL_BODY";
const NO_SELECTED_SKILLS_MESSAGE: &str = "No selected-environment skills are currently available.";
const MCP_SERVER_NAME: &str = "executor_probe";
const MCP_CALL_ID: &str = "selected-executor-mcp-call";
const CONNECTOR_ID: &str = "calendar";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_capability_stack_tracks_environment_availability_and_resume() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (apps_url, apps_server_handle) = start_apps_server_with_delays(
        vec![AppInfo {
            id: CONNECTOR_ID.to_string(),
            name: "Calendar".to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            icon_assets: None,
            icon_dark_assets: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        }],
        vec![connector_tool(CONNECTOR_ID, "Calendar")?],
        Duration::ZERO,
        Duration::ZERO,
    )
    .await?;
    let fixture = selected_capability_fixture(&responses_server.uri(), &apps_url)?;

    let response_mock = responses::mount_sse_sequence(
        &responses_server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("environment-unavailable"),
                responses::ev_assistant_message("unavailable-message", "Waiting"),
                responses::ev_completed("environment-unavailable"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("environment-available-call"),
                responses::ev_function_call_with_namespace(
                    MCP_CALL_ID,
                    &format!("mcp__{MCP_SERVER_NAME}"),
                    "echo",
                    &json!({
                        "message": "hello from the selected executor",
                        "env_var": EXECUTOR_ENV_NAME,
                    })
                    .to_string(),
                ),
                responses::ev_completed("environment-available-call"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("environment-available-done"),
                responses::ev_assistant_message("available-message", "Done"),
                responses::ev_completed("environment-available-done"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("unchanged-step"),
                responses::ev_assistant_message("unchanged-message", "Still ready"),
                responses::ev_completed("unchanged-step"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resumed-unavailable-step"),
                responses::ev_assistant_message(
                    "resumed-unavailable-message",
                    "Unavailable after resume",
                ),
                responses::ev_completed("resumed-unavailable-step"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("reattached-step"),
                responses::ev_assistant_message("reattached-message", "Ready after reattach"),
                responses::ev_completed("reattached-step"),
            ]),
        ],
    )
    .await;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(fixture.codex_home.path())
        // This fixture owns environments.toml and selects its environments explicitly.
        .without_auto_env()
        .build()
        .await?;
    timeout(READ_TIMEOUT, app_server.initialize()).await??;
    let thread_id = start_thread(
        &mut app_server,
        fixture.selected_root.clone(),
        fixture.environment_cwd.clone(),
    )
    .await?;

    run_turn(
        &mut app_server,
        &thread_id,
        "Inspect the current capabilities",
        fixture.environment_cwd.clone(),
    )
    .await?;
    let initial_requests = response_mock.requests();
    assert_selected_capabilities_absent(&initial_requests[0]);

    let mut exec_server =
        spawn_exec_server(fixture.codex_home.path(), &fixture.exec_server_url).await?;
    add_environment(&mut app_server, &fixture.exec_server_url).await?;
    wait_for_selected_mcp_server(&mut app_server, &thread_id).await?;

    run_turn(
        &mut app_server,
        &thread_id,
        &format!("Use ${SKILL_NAME} and call its selected executor MCP"),
        fixture.environment_cwd.clone(),
    )
    .await?;
    let first_mcp_pid = wait_for_pid_file(&fixture.pid_file).await?;

    run_turn(
        &mut app_server,
        &thread_id,
        "Continue with the same selected capabilities",
        fixture.environment_cwd.clone(),
    )
    .await?;
    assert_eq!(first_mcp_pid, wait_for_pid_file(&fixture.pid_file).await?);

    exec_server.kill().await?;
    drop(app_server);
    std::fs::remove_file(&fixture.pid_file)?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(fixture.codex_home.path())
        // This fixture owns environments.toml and selects its environments explicitly.
        .without_auto_env()
        .build()
        .await?;
    timeout(READ_TIMEOUT, app_server.initialize()).await??;
    let request_id = app_server
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            ..Default::default()
        })
        .await?;
    let response = timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadResumeResponse { thread, .. } = to_response(response)?;
    assert_eq!(thread_id, thread.id);

    run_turn(
        &mut app_server,
        &thread_id,
        "Inspect capabilities while the selected executor is unavailable",
        fixture.environment_cwd.clone(),
    )
    .await?;
    let requests = response_mock.requests();
    assert_eq!(5, requests.len());
    assert_selected_plugin_tools_absent(&requests[4]);
    assert!(
        latest_selected_skill_update(&requests[4])
            .is_some_and(|text| text.contains(NO_SELECTED_SKILLS_MESSAGE))
    );

    exec_server = spawn_exec_server(fixture.codex_home.path(), &fixture.exec_server_url).await?;
    add_environment(&mut app_server, &fixture.exec_server_url).await?;
    wait_for_selected_mcp_server(&mut app_server, &thread_id).await?;

    run_turn(
        &mut app_server,
        &thread_id,
        &format!("Use ${SKILL_NAME} after reattaching the selected executor"),
        fixture.environment_cwd,
    )
    .await?;
    let resumed_mcp_pid = wait_for_pid_file(&fixture.pid_file).await?;
    assert_ne!(first_mcp_pid, resumed_mcp_pid);

    let requests = response_mock.requests();
    assert_eq!(6, requests.len());
    for request in &requests[1..4] {
        assert_selected_skill_is_injected(request, /*expected_count*/ 1);
        assert_selected_plugin_tools(request);
        assert_plugin_guidance_count(request, /*expected_count*/ 1);
    }
    assert_plugin_guidance_count(&requests[4], /*expected_count*/ 1);
    assert_selected_skill_is_injected(&requests[5], /*expected_count*/ 2);
    assert_selected_plugin_tools(&requests[5]);
    let output = requests[2].function_call_output(MCP_CALL_ID);
    let output = output["output"]
        .as_str()
        .expect("MCP function output should be text");
    assert!(output.contains("ECHOING: hello from the selected executor"));
    assert!(output.contains(EXECUTOR_ENV_VALUE));

    exec_server.kill().await?;
    apps_server_handle.abort();
    let _ = apps_server_handle.await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_capabilities_become_available_between_samples_in_one_turn() -> Result<()> {
    const USER_INPUT_CALL_ID: &str = "pause-for-environment";

    let responses_server = responses::start_mock_server().await;
    let (apps_url, apps_server_handle) = start_apps_server_with_delays(
        vec![AppInfo {
            id: CONNECTOR_ID.to_string(),
            name: "Calendar".to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            icon_assets: None,
            icon_dark_assets: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        }],
        vec![connector_tool(CONNECTOR_ID, "Calendar")?],
        Duration::ZERO,
        Duration::ZERO,
    )
    .await?;
    let fixture = selected_capability_fixture(&responses_server.uri(), &apps_url)?;
    let response_mock = responses::mount_sse_sequence(
        &responses_server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("environment-pending"),
                responses::ev_function_call(
                    USER_INPUT_CALL_ID,
                    "request_user_input",
                    &json!({
                        "questions": [{
                            "id": "continue",
                            "header": "Continue",
                            "question": "Continue after the executor is attached?",
                            "options": [{
                                "label": "Yes (Recommended)",
                                "description": "Continue the same turn."
                            }, {
                                "label": "No",
                                "description": "Stop here."
                            }]
                        }],
                        "autoResolutionMs": 60_000
                    })
                    .to_string(),
                ),
                responses::ev_completed("environment-pending"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("environment-ready-call"),
                responses::ev_function_call_with_namespace(
                    MCP_CALL_ID,
                    &format!("mcp__{MCP_SERVER_NAME}"),
                    "echo",
                    &json!({
                        "message": "same turn",
                        "env_var": EXECUTOR_ENV_NAME,
                    })
                    .to_string(),
                ),
                responses::ev_completed("environment-ready-call"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("same-turn-done"),
                responses::ev_assistant_message("same-turn-message", "Done"),
                responses::ev_completed("same-turn-done"),
            ]),
        ],
    )
    .await;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(fixture.codex_home.path())
        // This fixture owns environments.toml and selects its environments explicitly.
        .without_auto_env()
        .build()
        .await?;
    timeout(READ_TIMEOUT, app_server.initialize()).await??;
    let thread_id = start_thread(
        &mut app_server,
        fixture.selected_root,
        fixture.environment_cwd.clone(),
    )
    .await?;
    let turn_start_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id,
            input: vec![UserInput::Text {
                text: "Use the executor when it becomes ready.".to_string(),
                text_elements: Vec::new(),
            }],
            environments: Some(vec![TurnEnvironmentParams {
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd: fixture.environment_cwd.into(),
            }]),
            collaboration_mode: Some(CollaborationMode {
                mode: ModeKind::Plan,
                settings: Settings {
                    model: "mock-model".to_string(),
                    reasoning_effort: None,
                    developer_instructions: None,
                },
            }),
            ..Default::default()
        })
        .await?;
    timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;

    let request = timeout(READ_TIMEOUT, app_server.read_stream_until_request_message()).await??;
    let ServerRequest::ToolRequestUserInput { request_id, .. } = request else {
        panic!("expected request_user_input, got {request:?}");
    };
    let requests = response_mock.requests();
    assert_eq!(1, requests.len());
    assert_selected_capabilities_absent(&requests[0]);

    let mut exec_server =
        spawn_exec_server(fixture.codex_home.path(), &fixture.exec_server_url).await?;
    add_environment(&mut app_server, &fixture.exec_server_url).await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    app_server
        .send_response(
            request_id,
            json!({
                "answers": {
                    "continue": { "answers": ["yes"] }
                }
            }),
        )
        .await?;
    timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = response_mock.requests();
    assert_eq!(3, requests.len());
    assert_selected_skill_catalog_available(&requests[1]);
    assert_selected_plugin_tools(&requests[1]);
    assert_plugin_guidance_count(&requests[1], /*expected_count*/ 1);
    assert_selected_plugin_tools(&requests[2]);
    assert_plugin_guidance_count(&requests[2], /*expected_count*/ 1);
    let output = requests[2].function_call_output(MCP_CALL_ID);
    let output = output["output"]
        .as_str()
        .expect("MCP function output should be text");
    assert!(output.contains("ECHOING: same turn"));
    assert!(output.contains(EXECUTOR_ENV_VALUE));
    wait_for_pid_file(&fixture.pid_file).await?;

    exec_server.kill().await?;
    apps_server_handle.abort();
    let _ = apps_server_handle.await;
    Ok(())
}

struct SelectedCapabilityFixture {
    codex_home: TempDir,
    _plugin: TempDir,
    pid_file: std::path::PathBuf,
    exec_server_url: String,
    selected_root: SelectedCapabilityRoot,
    environment_cwd: AbsolutePathBuf,
}

fn selected_capability_fixture(
    responses_server_uri: &str,
    apps_url: &str,
) -> Result<SelectedCapabilityFixture> {
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml_with_chatgpt_base_url(
        codex_home.path(),
        responses_server_uri,
        apps_url,
    )?;
    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?.replacen(
        "model_provider = \"mock_provider\"",
        "mcp_oauth_credentials_store = \"file\"\nmodel_provider = \"mock_provider\"",
        1,
    );
    std::fs::write(
        config_path,
        format!(
            "{config}\n[features]\napps = true\ndeferred_executor = true\n\n[skills]\ninclude_instructions = true\n"
        ),
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .email("selected-capability-stack@example.com")
            .plan_type("pro")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    // Reserve the URL before app-server starts. The configured environment initially fails to
    // connect, then environment/add points the same stable ID at the same URL once it is live.
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let exec_server_url = format!("ws://{}", listener.local_addr()?);
    drop(listener);
    std::fs::write(
        codex_home.path().join("environments.toml"),
        format!(
            "default = \"{EXECUTOR_ID}\"\ninclude_local = true\n\n[[environments]]\nid = \"{EXECUTOR_ID}\"\nurl = \"{exec_server_url}\"\nconnect_timeout_sec = 0.05\n"
        ),
    )?;

    let local_skill_dir = codex_home.path().join("skills/local-deploy");
    std::fs::create_dir_all(&local_skill_dir)?;
    std::fs::write(
        local_skill_dir.join("SKILL.md"),
        format!(
            "---\nname: {SKILL_NAME}\ndescription: Colliding local skill.\n---\n\n{LOCAL_SKILL_BODY_MARKER}\n"
        ),
    )?;

    let plugin = TempDir::new()?;
    let manifest_dir = plugin.path().join(".codex-plugin");
    let skill_dir = plugin.path().join("skills/deploy");
    let pid_file = plugin.path().join("executor-mcp.pid");
    std::fs::create_dir_all(&manifest_dir)?;
    std::fs::create_dir_all(&skill_dir)?;
    std::fs::write(
        manifest_dir.join("plugin.json"),
        r#"{"name":"executor-demo","apps":"./.app.json","interface":{"displayName":"Executor Demo"}}"#,
    )?;
    std::fs::write(
        skill_dir.join("SKILL.md"),
        format!(
            "---\nname: deploy\ndescription: {SKILL_DESCRIPTION}\n---\n\n{SKILL_BODY_MARKER}\n"
        ),
    )?;
    std::fs::write(
        plugin.path().join(".app.json"),
        format!(r#"{{"apps":{{"calendar":{{"id":"{CONNECTOR_ID}"}}}}}}"#),
    )?;
    std::fs::write(
        plugin.path().join(".mcp.json"),
        serde_json::to_vec_pretty(&json!({
            "mcpServers": {
                (MCP_SERVER_NAME): {
                    "command": stdio_server_bin()?,
                    "env": {
                        "MCP_TEST_PID_FILE": pid_file.to_string_lossy(),
                    },
                    "env_vars": [EXECUTOR_ENV_NAME],
                    "startup_timeout_sec": 10,
                }
            }
        }))?,
    )?;

    let selected_root = SelectedCapabilityRoot {
        id: PLUGIN_ID.to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: EXECUTOR_ID.to_string(),
            path: PathUri::from_host_native_path(plugin.path())?,
        },
    };
    let environment_cwd = AbsolutePathBuf::try_from(plugin.path().to_path_buf())?;
    Ok(SelectedCapabilityFixture {
        codex_home,
        _plugin: plugin,
        pid_file,
        exec_server_url,
        selected_root,
        environment_cwd,
    })
}

fn assert_selected_capabilities_absent(request: &ResponsesRequest) {
    assert!(
        request
            .message_input_texts("developer")
            .into_iter()
            .all(|text| !text.contains(SKILL_DESCRIPTION))
    );
    assert_selected_plugin_tools_absent(request);
    assert_plugin_guidance_count(request, /*expected_count*/ 0);
}

fn assert_selected_plugin_tools_absent(request: &ResponsesRequest) {
    assert!(
        request
            .tool_by_name(&format!("mcp__{MCP_SERVER_NAME}"), "echo")
            .is_none()
    );
    let connector = request
        .tool_by_name("mcp__codex_apps__calendar", "connector_calendar")
        .expect("host connector should remain model-visible");
    assert!(
        connector["description"]
            .as_str()
            .is_some_and(|description| !description.contains(PLUGIN_DISPLAY_NAME))
    );
}

fn assert_plugin_guidance_count(request: &ResponsesRequest, expected_count: usize) {
    assert_eq!(
        expected_count,
        request
            .message_input_texts("developer")
            .into_iter()
            .filter(|text| text.starts_with(PLUGINS_INSTRUCTIONS_OPEN_TAG))
            .count()
    );
}

fn assert_selected_skill_is_injected(request: &ResponsesRequest, expected_count: usize) {
    assert_selected_skill_catalog_available(request);

    let skill_fragments = request
        .message_input_texts("user")
        .into_iter()
        .filter(|text| text.starts_with("<skill>"))
        .collect::<Vec<_>>();
    assert_eq!(expected_count, skill_fragments.len());
    for fragment in skill_fragments {
        assert!(fragment.contains(&format!("<name>{SKILL_NAME}</name>")));
        assert!(fragment.contains(SKILL_BODY_MARKER));
        assert!(!fragment.contains(LOCAL_SKILL_BODY_MARKER));
    }
}

fn assert_selected_skill_catalog_available(request: &ResponsesRequest) {
    let catalog_fragment = latest_selected_skill_update(request)
        .expect("selected skill catalog update should be model-visible");
    assert!(catalog_fragment.contains(SKILL_DESCRIPTION));
    assert!(catalog_fragment.contains("environment resource:"));
}

fn latest_selected_skill_update(request: &ResponsesRequest) -> Option<String> {
    request
        .message_input_texts("developer")
        .into_iter()
        .rfind(|text| text.contains(SKILL_DESCRIPTION) || text.contains(NO_SELECTED_SKILLS_MESSAGE))
}

fn assert_selected_plugin_tools(request: &ResponsesRequest) {
    assert!(
        request
            .tool_by_name(&format!("mcp__{MCP_SERVER_NAME}"), "echo")
            .is_some()
    );
    let connector = request
        .tool_by_name("mcp__codex_apps__calendar", "connector_calendar")
        .expect("selected connector should be model-visible");
    assert!(
        connector["description"]
            .as_str()
            .is_some_and(|description| description.contains(PLUGIN_DISPLAY_NAME))
    );
}

async fn start_thread(
    app_server: &mut TestAppServer,
    selected_root: SelectedCapabilityRoot,
    environment_cwd: AbsolutePathBuf,
) -> Result<String> {
    let request_id = app_server
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            environments: Some(vec![TurnEnvironmentParams {
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd: environment_cwd.into(),
            }]),
            selected_capability_roots: Some(vec![selected_root]),
            ..Default::default()
        })
        .await?;
    let response = timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(response)?;
    Ok(thread.id)
}

async fn run_turn(
    app_server: &mut TestAppServer,
    thread_id: &str,
    text: &str,
    environment_cwd: AbsolutePathBuf,
) -> Result<()> {
    let request_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.to_string(),
            input: vec![UserInput::Text {
                text: text.to_string(),
                text_elements: Vec::new(),
            }],
            environments: Some(vec![TurnEnvironmentParams {
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd: environment_cwd.into(),
            }]),
            ..Default::default()
        })
        .await?;
    timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    Ok(())
}

async fn add_environment(app_server: &mut TestAppServer, exec_server_url: &str) -> Result<()> {
    let request_id = app_server
        .send_raw_request(
            "environment/add",
            Some(json!({
                "environmentId": EXECUTOR_ID,
                "execServerUrl": exec_server_url,
                "connectTimeoutMs": 10_000,
            })),
        )
        .await?;
    let response = timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let _: EnvironmentAddResponse = to_response(response)?;
    Ok(())
}

async fn wait_for_selected_mcp_server(
    app_server: &mut TestAppServer,
    thread_id: &str,
) -> Result<()> {
    timeout(READ_TIMEOUT, async {
        loop {
            let request_id = app_server
                .send_list_mcp_server_status_request(ListMcpServerStatusParams {
                    cursor: None,
                    limit: None,
                    detail: None,
                    thread_id: Some(thread_id.to_string()),
                })
                .await?;
            let response = app_server
                .read_stream_until_response_message(RequestId::Integer(request_id))
                .await?;
            let response: ListMcpServerStatusResponse = to_response(response)?;
            if response
                .data
                .iter()
                .any(|server| server.name == MCP_SERVER_NAME)
            {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    Ok(())
}

async fn spawn_exec_server(codex_home: &std::path::Path, url: &str) -> Result<Child> {
    let mut child = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .args(["exec-server", "--listen", url])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .env("CODEX_HOME", codex_home)
        .env(EXECUTOR_ENV_NAME, EXECUTOR_ENV_VALUE)
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .context("exec-server stdout was not captured")?;
    let mut lines = BufReader::new(stdout).lines();
    loop {
        let line = timeout(READ_TIMEOUT, lines.next_line())
            .await
            .context("timed out waiting for exec-server URL")??
            .context("exec-server exited before printing its URL")?;
        if line.trim() == url {
            return Ok(child);
        }
    }
}
