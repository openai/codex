use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_mock_responses_config_toml_with_chatgpt_base_url;
use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::AppsListParams;
use codex_app_server_protocol::AppsListResponse;
use codex_app_server_protocol::CapabilityRootLocation;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::ListMcpServerStatusResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SelectedCapabilityRoot;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use codex_exec_server::CreateDirectoryOptions;
use core_test_support::responses;
use core_test_support::test_codex::test_env;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

use super::app_list::connector_tool;
use super::app_list::start_apps_server_with_delays;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const ACCESS_TOKEN: &str = "chatgpt-token";
const APP_CONFIG: &[u8] = br#"{"apps":{"calendar_app":{"id":"calendar"}}}"#;
const CONFLICTING_MCP_CONFIG: &[u8] =
    br#"{"mcpServers":{"calendar_app":{"command":"must-not-start","startup_timeout_sec":1}}}"#;
const CONNECTOR_ID: &str = "calendar";
const CONNECTOR_NAME: &str = "Calendar";
const PLUGIN_DISPLAY_NAME: &str = "Executor Calendar";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_executor_connector_is_listed_hosted_and_suppresses_mcp_fallback() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let (apps_url, apps_server_handle) = start_apps_server_with_delays(
        vec![AppInfo {
            is_accessible: false,
            install_url: None,
            ..expected_app(Vec::new())
        }],
        vec![connector_tool(CONNECTOR_ID, CONNECTOR_NAME)?],
        Duration::ZERO,
        Duration::ZERO,
    )
    .await?;

    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml_with_chatgpt_base_url(
        codex_home.path(),
        &responses_server.uri(),
        &apps_url,
    )?;
    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?.replacen(
        "model_provider = \"mock_provider\"",
        "mcp_oauth_credentials_store = \"file\"\nmodel_provider = \"mock_provider\"",
        1,
    );
    std::fs::write(config_path, format!("{config}\n[features]\napps = true\n"))?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new(ACCESS_TOKEN)
            .account_id("account-123")
            .email("executor-connectors@example.com")
            .plan_type("pro")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let executor_fixture = test_env().await?;
    let executor_file_system = executor_fixture.environment().get_filesystem();
    let plugin_root = executor_fixture.selection().cwd.join("executor-calendar")?;
    let manifest_dir = plugin_root.join(".codex-plugin")?;
    executor_file_system
        .create_directory(
            &manifest_dir,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;
    executor_file_system
        .write_file(
            &manifest_dir.join("plugin.json")?,
            br#"{"name":"executor-calendar","apps":"./.app.json","interface":{"displayName":"Executor Calendar"}}"#
                .to_vec(),
            /*sandbox*/ None,
        )
        .await?;
    executor_file_system
        .write_file(
            &plugin_root.join(".app.json")?,
            APP_CONFIG.to_vec(),
            /*sandbox*/ None,
        )
        .await?;
    executor_file_system
        .write_file(
            &plugin_root.join(".mcp.json")?,
            CONFLICTING_MCP_CONFIG.to_vec(),
            /*sandbox*/ None,
        )
        .await?;

    let mut app_server = TestAppServer::new_with_auto_env(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, app_server.initialize()).await??;
    let environment_id = app_server.auto_env_params()?.environment_id;
    let thread_start_id = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            selected_capability_roots: Some(vec![SelectedCapabilityRoot {
                id: "executor-calendar@1".to_string(),
                location: CapabilityRootLocation::Environment {
                    environment_id,
                    path: plugin_root,
                },
            }]),
            ..Default::default()
        })
        .await?;
    let thread_start_response = timeout(
        DEFAULT_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(thread_start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_start_response)?;
    let thread_id = thread.id;

    let apps_list_id = app_server
        .send_apps_list_request(AppsListParams {
            cursor: None,
            limit: None,
            thread_id: Some(thread_id.clone()),
            force_refetch: true,
        })
        .await?;
    let apps_list_response = timeout(
        DEFAULT_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(apps_list_id)),
    )
    .await??;
    assert_eq!(
        to_response::<AppsListResponse>(apps_list_response)?,
        AppsListResponse {
            data: vec![expected_app(vec![PLUGIN_DISPLAY_NAME.to_string()])],
            next_cursor: None,
        }
    );

    let mcp_status_id = app_server
        .send_list_mcp_server_status_request(ListMcpServerStatusParams {
            cursor: None,
            limit: None,
            detail: None,
            thread_id: Some(thread_id.clone()),
        })
        .await?;
    let mcp_status_response = timeout(
        DEFAULT_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(mcp_status_id)),
    )
    .await??;
    let mcp_status_response: ListMcpServerStatusResponse = to_response(mcp_status_response)?;
    assert!(
        mcp_status_response
            .data
            .iter()
            .all(|server| server.name != "calendar_app")
    );

    let response_mock = responses::mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "Done"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let turn_start_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id,
            input: vec![UserInput::Text {
                text: "Use Calendar".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(turn_start_id)),
    )
    .await??;
    timeout(
        DEFAULT_TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let description = response_mock
        .single_request()
        .tool_by_name("mcp__codex_apps__calendar", "connector_calendar")
        .context("Calendar connector tool should be model-visible")?
        .get("description")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .context("Calendar connector tool should have a description")?;
    assert!(description.contains("This tool is part of plugin `Executor Calendar`."));

    apps_server_handle.abort();
    let _ = apps_server_handle.await;
    Ok(())
}

fn expected_app(plugin_display_names: Vec<String>) -> AppInfo {
    AppInfo {
        id: CONNECTOR_ID.to_string(),
        name: CONNECTOR_NAME.to_string(),
        description: Some("Calendar connector".to_string()),
        logo_url: None,
        logo_url_dark: None,
        distribution_channel: None,
        branding: None,
        app_metadata: None,
        labels: None,
        install_url: Some(codex_connectors::metadata::connector_install_url(
            CONNECTOR_NAME,
            CONNECTOR_ID,
        )),
        is_accessible: true,
        is_enabled: true,
        plugin_display_names,
    }
}
