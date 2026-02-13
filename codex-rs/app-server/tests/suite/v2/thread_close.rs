use anyhow::Context;
use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_shell_command_sse_response;
use app_test_support::to_response;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadCloseParams;
use codex_app_server_protocol::ThreadCloseResponse;
use codex_app_server_protocol::ThreadCloseStatus;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput as V2UserInput;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn thread_close_unloads_thread_and_emits_thread_closed_notification() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_id = start_thread(&mut mcp).await?;

    let close_id = mcp
        .send_thread_close_request(ThreadCloseParams {
            thread_id: thread_id.clone(),
        })
        .await?;
    let close_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(close_id)),
    )
    .await??;
    let close = to_response::<ThreadCloseResponse>(close_resp)?;
    assert_eq!(close.status, ThreadCloseStatus::Closed);

    let closed_notif: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/closed"),
    )
    .await??;
    let parsed: ServerNotification = closed_notif.try_into()?;
    let ServerNotification::ThreadClosed(payload) = parsed else {
        anyhow::bail!("expected thread/closed notification");
    };
    assert_eq!(payload.thread_id, thread_id);

    let list_id = mcp
        .send_thread_loaded_list_request(ThreadLoadedListParams::default())
        .await?;
    let list_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(list_id)),
    )
    .await??;
    let ThreadLoadedListResponse { data, next_cursor } =
        to_response::<ThreadLoadedListResponse>(list_resp)?;
    assert_eq!(data, Vec::<String>::new());
    assert_eq!(next_cursor, None);

    Ok(())
}

#[tokio::test]
async fn thread_close_during_turn_interrupts_turn_and_emits_thread_closed() -> Result<()> {
    #[cfg(target_os = "windows")]
    let shell_command = vec![
        "powershell".to_string(),
        "-Command".to_string(),
        "Start-Sleep -Seconds 10".to_string(),
    ];
    #[cfg(not(target_os = "windows"))]
    let shell_command = vec!["sleep".to_string(), "10".to_string()];

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let working_directory = tmp.path().join("workdir");
    std::fs::create_dir(&working_directory)?;

    let server = create_mock_responses_server_sequence(vec![create_shell_command_sse_response(
        shell_command.clone(),
        Some(&working_directory),
        Some(10_000),
        "call_sleep",
    )?])
    .await;
    create_config_toml(&codex_home, &server.uri())?;

    let mut mcp = McpProcess::new(&codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_id = start_thread(&mut mcp).await?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![V2UserInput::Text {
                text: "run sleep".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(working_directory),
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_req)),
    )
    .await??;
    let _: TurnStartResponse = to_response::<TurnStartResponse>(turn_resp)?;

    timeout(
        DEFAULT_READ_TIMEOUT,
        wait_for_command_execution_item_started(&mut mcp),
    )
    .await??;

    let close_id = mcp
        .send_thread_close_request(ThreadCloseParams {
            thread_id: thread_id.clone(),
        })
        .await?;
    let close_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(close_id)),
    )
    .await??;
    let close = to_response::<ThreadCloseResponse>(close_resp)?;
    assert_eq!(close.status, ThreadCloseStatus::Closed);

    let completed_notif: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    let completed_params = completed_notif
        .params
        .context("turn/completed params must be present")?;
    let completed: TurnCompletedNotification = serde_json::from_value(completed_params)?;
    assert_eq!(completed.thread_id, thread_id);
    assert_eq!(completed.turn.status, TurnStatus::Interrupted);

    let closed_notif: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/closed"),
    )
    .await??;
    let parsed: ServerNotification = closed_notif.try_into()?;
    let ServerNotification::ThreadClosed(payload) = parsed else {
        anyhow::bail!("expected thread/closed notification");
    };
    assert_eq!(payload.thread_id, completed.thread_id);

    Ok(())
}

async fn wait_for_command_execution_item_started(mcp: &mut McpProcess) -> Result<()> {
    loop {
        let started_notif = mcp
            .read_stream_until_notification_message("item/started")
            .await?;
        let started_params = started_notif.params.context("item/started params")?;
        let started: ItemStartedNotification = serde_json::from_value(started_params)?;
        if let ThreadItem::CommandExecution { .. } = started.item {
            return Ok(());
        }
    }
}

fn create_config_toml(codex_home: &std::path::Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}

async fn start_thread(mcp: &mut McpProcess) -> Result<String> {
    let req_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(resp)?;
    Ok(thread.id)
}
