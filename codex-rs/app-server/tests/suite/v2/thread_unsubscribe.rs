use anyhow::Context;
use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::create_shell_command_sse_response_from_command;
use app_test_support::to_response;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::ThreadUnsubscribeStatus;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

async fn wait_for_responses_request_count_to_stabilize(
    server: &wiremock::MockServer,
    expected_count: usize,
    settle_duration: std::time::Duration,
) -> Result<()> {
    timeout(DEFAULT_READ_TIMEOUT, async {
        let mut stable_since: Option<tokio::time::Instant> = None;
        loop {
            let requests = server
                .received_requests()
                .await
                .context("failed to fetch received requests")?;
            let responses_request_count = requests
                .iter()
                .filter(|request| {
                    request.method == "POST" && request.url.path().ends_with("/responses")
                })
                .count();

            if responses_request_count > expected_count {
                anyhow::bail!(
                    "expected exactly {expected_count} /responses requests, got {responses_request_count}"
                );
            }

            if responses_request_count == expected_count {
                match stable_since {
                    Some(stable_since) if stable_since.elapsed() >= settle_duration => {
                        return Ok::<(), anyhow::Error>(());
                    }
                    None => stable_since = Some(tokio::time::Instant::now()),
                    Some(_) => {}
                }
            } else {
                stable_since = None;
            }

            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await??;

    Ok(())
}

#[tokio::test]
async fn thread_unsubscribe_keeps_thread_loaded_until_idle_timeout() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_id = start_thread(&mut mcp).await?;

    let unsubscribe_id = mcp
        .send_thread_unsubscribe_request(ThreadUnsubscribeParams {
            thread_id: thread_id.clone(),
        })
        .await?;
    let unsubscribe_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(unsubscribe_id)),
    )
    .await??;
    let unsubscribe = to_response::<ThreadUnsubscribeResponse>(unsubscribe_resp)?;
    assert_eq!(unsubscribe.status, ThreadUnsubscribeStatus::Unsubscribed);

    assert!(
        timeout(
            std::time::Duration::from_millis(250),
            mcp.read_stream_until_notification_message("thread/closed"),
        )
        .await
        .is_err()
    );

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
    assert_eq!(data, vec![thread_id]);
    assert_eq!(next_cursor, None);

    Ok(())
}

#[tokio::test]
async fn thread_unsubscribe_during_turn_keeps_turn_running() -> Result<()> {
    #[cfg(target_os = "windows")]
    let shell_command = "Start-Sleep -Seconds 1";
    #[cfg(not(target_os = "windows"))]
    let shell_command = "sleep 1";

    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let working_directory = tmp.path().join("workdir");
    std::fs::create_dir(&working_directory)?;

    let server = create_mock_responses_server_sequence_unchecked(vec![
        create_shell_command_sse_response_from_command(
            shell_command,
            Some(&working_directory),
            Some(10_000),
            "call_sleep",
            Some(false),
        )?,
        create_final_assistant_message_sse_response("Done")?,
    ])
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

    let unsubscribe_id = mcp
        .send_thread_unsubscribe_request(ThreadUnsubscribeParams {
            thread_id: thread_id.clone(),
        })
        .await?;
    let unsubscribe_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(unsubscribe_id)),
    )
    .await??;
    let unsubscribe = to_response::<ThreadUnsubscribeResponse>(unsubscribe_resp)?;
    assert_eq!(unsubscribe.status, ThreadUnsubscribeStatus::Unsubscribed);

    assert!(
        timeout(
            std::time::Duration::from_millis(250),
            mcp.read_stream_until_notification_message("thread/closed"),
        )
        .await
        .is_err()
    );

    wait_for_responses_request_count_to_stabilize(
        &server,
        /*expected_count*/ 2,
        std::time::Duration::from_millis(200),
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn thread_unsubscribe_preserves_cached_status_before_idle_unload() -> Result<()> {
    let server = responses::start_mock_server().await;
    let _response_mock = responses::mount_sse_once(
        &server,
        responses::sse_failed("resp-1", "server_error", "simulated failure"),
    )
    .await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_id = start_thread(&mut mcp).await?;

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![V2UserInput::Text {
                text: "fail this turn".to_string(),
                text_elements: Vec::new(),
            }],
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
        mcp.read_stream_until_notification_message("error"),
    )
    .await??;

    let read_id = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread_id.clone(),
            include_turns: false,
        })
        .await?;
    let read_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_id)),
    )
    .await??;
    let ThreadReadResponse { thread } = to_response::<ThreadReadResponse>(read_resp)?;
    assert_eq!(thread.status, ThreadStatus::SystemError);

    let unsubscribe_id = mcp
        .send_thread_unsubscribe_request(ThreadUnsubscribeParams {
            thread_id: thread_id.clone(),
        })
        .await?;
    let unsubscribe_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(unsubscribe_id)),
    )
    .await??;
    let unsubscribe = to_response::<ThreadUnsubscribeResponse>(unsubscribe_resp)?;
    assert_eq!(unsubscribe.status, ThreadUnsubscribeStatus::Unsubscribed);
    assert!(
        timeout(
            std::time::Duration::from_millis(250),
            mcp.read_stream_until_notification_message("thread/closed"),
        )
        .await
        .is_err()
    );

    let resume_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id,
            ..Default::default()
        })
        .await?;
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_id)),
    )
    .await??;
    let resume: ThreadResumeResponse = to_response::<ThreadResumeResponse>(resume_resp)?;
    assert_eq!(resume.thread.status, ThreadStatus::SystemError);

    Ok(())
}

#[tokio::test]
async fn thread_unsubscribe_reports_not_subscribed_before_idle_unload() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_id = start_thread(&mut mcp).await?;

    let first_unsubscribe_id = mcp
        .send_thread_unsubscribe_request(ThreadUnsubscribeParams {
            thread_id: thread_id.clone(),
        })
        .await?;
    let first_unsubscribe_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(first_unsubscribe_id)),
    )
    .await??;
    let first_unsubscribe = to_response::<ThreadUnsubscribeResponse>(first_unsubscribe_resp)?;
    assert_eq!(
        first_unsubscribe.status,
        ThreadUnsubscribeStatus::Unsubscribed
    );

    let second_unsubscribe_id = mcp
        .send_thread_unsubscribe_request(ThreadUnsubscribeParams { thread_id })
        .await?;
    let second_unsubscribe_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(second_unsubscribe_id)),
    )
    .await??;
    let second_unsubscribe = to_response::<ThreadUnsubscribeResponse>(second_unsubscribe_resp)?;
    assert_eq!(
        second_unsubscribe.status,
        ThreadUnsubscribeStatus::NotSubscribed
    );

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
