use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadArchivedNotification;
use codex_app_server_protocol::ThreadClosedNotification;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::ThreadUnarchiveParams;
use codex_app_server_protocol::ThreadUnarchiveResponse;
use codex_app_server_protocol::ThreadUnarchivedNotification;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::ThreadUnsubscribeStatus;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::stdio_server_bin;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const STARTUP_COUNT_POLL_INTERVAL: Duration = Duration::from_millis(25);
const STARTUP_COUNT_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const STARTUP_COUNT_STABILITY_WINDOW: Duration = Duration::from_millis(250);
const STARTUP_COUNT_FILE_ENV_VAR: &str = "MCP_STARTUP_COUNT_FILE";

#[tokio::test]
async fn mcp_pool_survives_unsubscribe_of_one_loaded_thread() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses_server =
        create_mock_responses_server_sequence_unchecked(rmcp_echo_turn_bodies("after-unsubscribe"))
            .await;

    let codex_home = TempDir::new()?;
    let startup_count_file = codex_home.path().join("rmcp-startups.log");
    create_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        &stdio_server_bin()?,
    )?;

    let startup_count_file_value = startup_count_file.to_string_lossy().to_string();
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[(
            STARTUP_COUNT_FILE_ENV_VAR,
            Some(startup_count_file_value.as_str()),
        )],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_a = start_thread(&mut mcp).await?;
    wait_for_startup_count(&startup_count_file, 1).await?;

    let thread_b = start_thread(&mut mcp).await?;
    assert_startup_count_stays(&startup_count_file, 1, STARTUP_COUNT_STABILITY_WINDOW).await?;

    unsubscribe_thread(&mut mcp, &thread_a).await?;
    run_rmcp_echo_turn(&mut mcp, &thread_b, "after-unsubscribe").await?;
    assert_startup_count_stays(&startup_count_file, 1, STARTUP_COUNT_STABILITY_WINDOW).await?;

    Ok(())
}

#[tokio::test]
async fn mcp_pool_survives_archive_of_one_loaded_thread() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses_server = create_mock_responses_server_sequence_unchecked(
        [
            rmcp_echo_turn_bodies("materialize-archived-thread"),
            rmcp_echo_turn_bodies("after-archive"),
        ]
        .into_iter()
        .flatten()
        .collect(),
    )
    .await;

    let codex_home = TempDir::new()?;
    let startup_count_file = codex_home.path().join("rmcp-startups.log");
    create_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        &stdio_server_bin()?,
    )?;

    let startup_count_file_value = startup_count_file.to_string_lossy().to_string();
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[(
            STARTUP_COUNT_FILE_ENV_VAR,
            Some(startup_count_file_value.as_str()),
        )],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_a = start_thread(&mut mcp).await?;
    wait_for_startup_count(&startup_count_file, 1).await?;

    let thread_b = start_thread(&mut mcp).await?;
    assert_startup_count_stays(&startup_count_file, 1, STARTUP_COUNT_STABILITY_WINDOW).await?;

    run_rmcp_echo_turn(&mut mcp, &thread_a, "materialize-archived-thread").await?;
    archive_thread(&mut mcp, &thread_a).await?;

    run_rmcp_echo_turn(&mut mcp, &thread_b, "after-archive").await?;
    assert_startup_count_stays(&startup_count_file, 1, STARTUP_COUNT_STABILITY_WINDOW).await?;

    Ok(())
}

#[tokio::test]
async fn mcp_pool_recreates_backend_after_last_archive_and_resume() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses_server = create_mock_responses_server_sequence_unchecked(
        [
            rmcp_echo_turn_bodies("materialize-a"),
            rmcp_echo_turn_bodies("materialize-b"),
            rmcp_echo_turn_bodies("after-resume"),
        ]
        .into_iter()
        .flatten()
        .collect(),
    )
    .await;

    let codex_home = TempDir::new()?;
    let startup_count_file = codex_home.path().join("rmcp-startups.log");
    create_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        &stdio_server_bin()?,
    )?;

    let startup_count_file_value = startup_count_file.to_string_lossy().to_string();
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[(
            STARTUP_COUNT_FILE_ENV_VAR,
            Some(startup_count_file_value.as_str()),
        )],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_a = start_thread(&mut mcp).await?;
    wait_for_startup_count(&startup_count_file, 1).await?;

    let thread_b = start_thread(&mut mcp).await?;
    assert_startup_count_stays(&startup_count_file, 1, STARTUP_COUNT_STABILITY_WINDOW).await?;

    run_rmcp_echo_turn(&mut mcp, &thread_a, "materialize-a").await?;
    run_rmcp_echo_turn(&mut mcp, &thread_b, "materialize-b").await?;

    archive_thread(&mut mcp, &thread_a).await?;
    archive_thread(&mut mcp, &thread_b).await?;

    let unarchive = unarchive_thread(&mut mcp, &thread_a).await?;
    assert_eq!(unarchive.thread.status, ThreadStatus::NotLoaded);
    assert_startup_count_stays(&startup_count_file, 1, STARTUP_COUNT_STABILITY_WINDOW).await?;

    let resume = resume_thread(&mut mcp, &thread_a).await?;
    assert_eq!(resume.thread.status, ThreadStatus::Idle);
    wait_for_startup_count(&startup_count_file, 2).await?;

    run_rmcp_echo_turn(&mut mcp, &thread_a, "after-resume").await?;
    assert_startup_count_stays(&startup_count_file, 2, STARTUP_COUNT_STABILITY_WINDOW).await?;

    Ok(())
}

fn create_config_toml(codex_home: &Path, server_uri: &str, rmcp_server_bin: &str) -> Result<()> {
    let config_toml = codex_home.join("config.toml");
    let server_uri = serde_json::to_string(&format!("{server_uri}/v1"))?;
    let rmcp_server_bin = serde_json::to_string(rmcp_server_bin)?;
    std::fs::write(
        config_toml,
        format!(
            r#"model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = {server_uri}
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0

[mcp_servers.rmcp]
command = {rmcp_server_bin}
env_vars = ["{STARTUP_COUNT_FILE_ENV_VAR}"]
startup_timeout_sec = 10.0
"#
        ),
    )
    .context("write config.toml")
}

fn rmcp_echo_turn_bodies(label: &str) -> Vec<String> {
    let response_id = format!("resp-{label}");
    let completion_id = format!("resp-{label}-done");
    let call_id = format!("call-rmcp-echo-{label}");
    let assistant_message_id = format!("msg-{label}");
    let message = format!("ping-{label}");
    let final_text = format!("rmcp echo completed for {label}");
    let arguments = json!({ "message": message }).to_string();

    vec![
        responses::sse(vec![
            responses::ev_response_created(&response_id),
            responses::ev_function_call(&call_id, "mcp__rmcp__echo", &arguments),
            responses::ev_completed(&response_id),
        ]),
        responses::sse(vec![
            responses::ev_response_created(&completion_id),
            responses::ev_assistant_message(&assistant_message_id, &final_text),
            responses::ev_completed(&completion_id),
        ]),
    ]
}

async fn start_thread(mcp: &mut McpProcess) -> Result<String> {
    let request_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(response)?;
    Ok(thread.id)
}

async fn run_rmcp_echo_turn(mcp: &mut McpProcess, thread_id: &str, label: &str) -> Result<()> {
    let request_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.to_string(),
            input: vec![UserInput::Text {
                text: format!("call the rmcp echo tool for {label}"),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response(response)?;

    let deadline = Instant::now() + DEFAULT_READ_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let message = timeout(remaining, mcp.read_next_message()).await??;
        let codex_app_server_protocol::JSONRPCMessage::Notification(notification) = message else {
            continue;
        };
        if notification.method != "turn/completed" {
            continue;
        }

        let completed: TurnCompletedNotification = serde_json::from_value(
            notification
                .params
                .clone()
                .context("turn/completed params")?,
        )?;
        if completed.thread_id != thread_id || completed.turn.id != turn.id {
            continue;
        }

        assert_eq!(completed.turn.status, TurnStatus::Completed);
        return Ok(());
    }
}

async fn unsubscribe_thread(mcp: &mut McpProcess, thread_id: &str) -> Result<()> {
    let request_id = mcp
        .send_thread_unsubscribe_request(ThreadUnsubscribeParams {
            thread_id: thread_id.to_string(),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let unsubscribe: ThreadUnsubscribeResponse = to_response(response)?;
    assert_eq!(unsubscribe.status, ThreadUnsubscribeStatus::Unsubscribed);

    let notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/closed"),
    )
    .await??;
    let ServerNotification::ThreadClosed(ThreadClosedNotification {
        thread_id: closed_thread_id,
    }) = notification.try_into()?
    else {
        bail!("expected thread/closed notification");
    };
    assert_eq!(closed_thread_id, thread_id);
    Ok(())
}

async fn archive_thread(mcp: &mut McpProcess, thread_id: &str) -> Result<()> {
    let request_id = mcp
        .send_thread_archive_request(ThreadArchiveParams {
            thread_id: thread_id.to_string(),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let _: ThreadArchiveResponse = to_response(response)?;

    let notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/archived"),
    )
    .await??;
    let archived: ThreadArchivedNotification =
        serde_json::from_value(notification.params.context("thread/archived params")?)?;
    assert_eq!(archived.thread_id, thread_id);
    Ok(())
}

async fn unarchive_thread(
    mcp: &mut McpProcess,
    thread_id: &str,
) -> Result<ThreadUnarchiveResponse> {
    let request_id = mcp
        .send_thread_unarchive_request(ThreadUnarchiveParams {
            thread_id: thread_id.to_string(),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let unarchive: ThreadUnarchiveResponse = to_response(response)?;

    let notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/unarchived"),
    )
    .await??;
    let unarchived: ThreadUnarchivedNotification =
        serde_json::from_value(notification.params.context("thread/unarchived params")?)?;
    assert_eq!(unarchived.thread_id, thread_id);

    Ok(unarchive)
}

async fn resume_thread(mcp: &mut McpProcess, thread_id: &str) -> Result<ThreadResumeResponse> {
    let request_id = mcp
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.to_string(),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn wait_for_startup_count(path: &Path, expected: usize) -> Result<()> {
    let deadline = Instant::now() + STARTUP_COUNT_WAIT_TIMEOUT;
    loop {
        let actual = read_startup_count(path)?;
        if actual == expected {
            return Ok(());
        }
        if actual > expected {
            bail!("startup count exceeded expectation: expected {expected}, got {actual}");
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for startup count {expected}; last observed {actual}");
        }
        sleep(STARTUP_COUNT_POLL_INTERVAL).await;
    }
}

async fn assert_startup_count_stays(
    path: &Path,
    expected: usize,
    duration: Duration,
) -> Result<()> {
    let deadline = Instant::now() + duration;
    loop {
        let actual = read_startup_count(path)?;
        assert_eq!(actual, expected);
        if Instant::now() >= deadline {
            return Ok(());
        }
        sleep(STARTUP_COUNT_POLL_INTERVAL).await;
    }
}

fn read_startup_count(path: &Path) -> Result<usize> {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error).context("read startup count file"),
    };
    Ok(contents.lines().count())
}
