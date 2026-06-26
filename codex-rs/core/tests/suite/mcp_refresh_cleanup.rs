use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::time::Duration;

use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::CALENDAR_MCP_SERVER_NAME;
use core_test_support::apps_test_server::SEARCH_CALENDAR_LIST_TOOL;
use core_test_support::apps_test_server::apps_enabled_builder;
use core_test_support::process::process_is_alive;
use core_test_support::process::wait_for_pid_file;
use core_test_support::process::wait_for_process_exit;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;
use core_test_support::wait_for_mcp_server_registration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refresh_keeps_superseded_mcp_server_alive_for_in_flight_calls() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let temp_dir = tempfile::tempdir()?;
    let pid_file = temp_dir.path().join("mcp.pid");
    let pid_file_for_config = pid_file.clone();
    let command = stdio_server_bin()?;
    let fixture = test_codex()
        .with_config(move |config| {
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                "refresh_cleanup".to_string(),
                McpServerConfig {
                    auth: Default::default(),
                    transport: McpServerTransportConfig::Stdio {
                        command,
                        args: Vec::new(),
                        env: Some(HashMap::from([(
                            "MCP_TEST_PID_FILE".to_string(),
                            pid_file_for_config.to_string_lossy().into_owned(),
                        )])),
                        env_vars: Vec::new(),
                        cwd: None,
                    },
                    environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
                    enabled: true,
                    required: false,
                    supports_parallel_tool_calls: false,
                    disabled_reason: None,
                    startup_timeout_sec: Some(Duration::from_secs(10)),
                    tool_timeout_sec: None,
                    default_tools_approval_mode: None,
                    enabled_tools: None,
                    disabled_tools: None,
                    scopes: None,
                    oauth: None,
                    oauth_resource: None,
                    tools: HashMap::new(),
                },
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test MCP servers should accept any configuration");
        })
        .build(&server)
        .await?;
    wait_for_mcp_server(&fixture.codex, "refresh_cleanup").await?;

    let superseded_pid = wait_for_pid_file(&pid_file).await?;
    assert!(process_is_alive(&superseded_pid)?);

    let barrier = serde_json::json!({
        "id": "mcp-refresh-cleanup",
        "participants": 2,
        "timeout_ms": 1_000
    });
    let long_call = tokio::spawn({
        let codex = Arc::clone(&fixture.codex);
        let barrier = barrier.clone();
        async move {
            codex
                .call_mcp_tool(
                    "refresh_cleanup",
                    "sync",
                    Some(serde_json::json!({
                        "barrier": barrier,
                        "sleep_after_ms": 300_000
                    })),
                    /*meta*/ None,
                )
                .await
        }
    });
    fixture
        .codex
        .call_mcp_tool(
            "refresh_cleanup",
            "sync",
            Some(serde_json::json!({ "barrier": barrier })),
            /*meta*/ None,
        )
        .await?;
    fs::remove_file(&pid_file)?;

    responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "done"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    fixture
        .codex
        .set_openai_form_elicitation_support(/*supported*/ true)
        .await?;
    fixture.submit_turn("refresh MCP servers").await?;

    let replacement_pid = wait_for_pid_file(&pid_file).await?;
    assert_ne!(replacement_pid, superseded_pid);
    assert!(process_is_alive(&superseded_pid)?);
    long_call.abort();
    assert!(
        long_call
            .await
            .expect_err("call should be aborted")
            .is_cancelled()
    );
    wait_for_process_exit(&superseded_pid).await?;
    assert!(process_is_alive(&replacement_pid)?);

    fixture.codex.shutdown_and_wait().await?;
    wait_for_process_exit(&replacement_pid).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apps_publication_reuses_unrelated_stateful_mcp_server() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let temp_dir = tempfile::tempdir()?;
    let pid_file = temp_dir.path().join("stateful-mcp.pid");
    let pid_file_for_config = pid_file.clone();
    let command = stdio_server_bin()?;
    let fixture = apps_enabled_builder(apps_server.chatgpt_base_url.clone())
        .with_config(move |config| {
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                "stateful".to_string(),
                McpServerConfig {
                    auth: Default::default(),
                    transport: McpServerTransportConfig::Stdio {
                        command,
                        args: Vec::new(),
                        env: Some(HashMap::from([(
                            "MCP_TEST_PID_FILE".to_string(),
                            pid_file_for_config.to_string_lossy().into_owned(),
                        )])),
                        env_vars: Vec::new(),
                        cwd: None,
                    },
                    environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
                    enabled: true,
                    required: false,
                    supports_parallel_tool_calls: false,
                    disabled_reason: None,
                    startup_timeout_sec: Some(Duration::from_secs(10)),
                    tool_timeout_sec: None,
                    default_tools_approval_mode: None,
                    enabled_tools: None,
                    disabled_tools: None,
                    scopes: None,
                    oauth: None,
                    oauth_resource: None,
                    tools: HashMap::new(),
                },
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test MCP servers should accept any configuration");
        })
        .build(&server)
        .await?;
    wait_for_mcp_server(&fixture.codex, "stateful").await?;
    let initial_pid = wait_for_pid_file(&pid_file).await?;

    responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "done"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    fixture.submit_turn("publish Apps MCP servers").await?;
    wait_for_mcp_server_registration(&fixture.codex, CALENDAR_MCP_SERVER_NAME).await?;
    // `submit_turn` drains the startup summary emitted during the turn. Polling the newly
    // published server directly proves that its asynchronous startup completed.
    tokio::time::timeout(
        Duration::from_secs(10),
        fixture.codex.call_mcp_tool(
            CALENDAR_MCP_SERVER_NAME,
            SEARCH_CALENDAR_LIST_TOOL,
            Some(serde_json::json!({"query": "reuse proof"})),
            /*meta*/ None,
        ),
    )
    .await
    .map_err(|_| anyhow::anyhow!("timed out waiting for published Apps MCP server startup"))??;

    assert_eq!(wait_for_pid_file(&pid_file).await?, initial_pid);
    assert!(process_is_alive(&initial_pid)?);
    fixture.codex.shutdown_and_wait().await?;
    wait_for_process_exit(&initial_pid).await
}
