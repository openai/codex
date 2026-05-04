use anyhow::Context;
use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use codex_app_server_protocol::ProcessExitedNotification;
use codex_app_server_protocol::ProcessKillParams;
use codex_app_server_protocol::ProcessSpawnParams;
use codex_app_server_protocol::ProcessSpawnResponse;
use codex_app_server_protocol::RequestId;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::timeout;

use super::connection_handling_websocket::DEFAULT_READ_TIMEOUT;
use super::connection_handling_websocket::create_config_toml;

#[tokio::test]
async fn process_spawn_returns_before_exit_and_emits_exit_notification() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let process_handle = "one-shot-1".to_string();
    let probe_file = codex_home.path().join("process-created");
    let spawn_request_id = mcp
        .send_process_spawn_request(ProcessSpawnParams {
            command: vec![
                "sh".to_string(),
                "-c".to_string(),
                "printf process > \"$1\"; sleep 1; printf process-out; printf process-err >&2"
                    .to_string(),
                "sh".to_string(),
                probe_file.display().to_string(),
            ],
            process_handle: process_handle.clone(),
            cwd: AbsolutePathBuf::try_from(codex_home.path())?,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: Some(None),
            timeout_ms: Some(None),
            env: None,
            size: None,
        })
        .await?;

    let started_at = Instant::now();
    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(spawn_request_id))
        .await?;
    assert!(
        started_at.elapsed() < Duration::from_millis(900),
        "process/spawn should return before the process exits"
    );
    let response: ProcessSpawnResponse = to_response(response)?;
    assert_eq!(
        response,
        ProcessSpawnResponse {
            process_handle: process_handle.clone(),
        }
    );

    let exited = read_process_exited(&mut mcp).await?;
    assert_eq!(
        exited,
        ProcessExitedNotification {
            process_handle,
            exit_code: 0,
            stdout: "process-out".to_string(),
            stdout_cap_reached: false,
            stderr: "process-err".to_string(),
            stderr_cap_reached: false,
        }
    );
    assert_eq!(std::fs::read_to_string(probe_file)?, "process");

    Ok(())
}

#[tokio::test]
async fn process_spawn_reports_buffered_output_cap_reached() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let process_handle = "capped-one-shot-1".to_string();
    let spawn_request_id = mcp
        .send_process_spawn_request(ProcessSpawnParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "printf abcde; printf 12345 >&2".to_string(),
            ],
            process_handle: process_handle.clone(),
            cwd: AbsolutePathBuf::try_from(codex_home.path())?,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: Some(Some(3)),
            timeout_ms: None,
            env: None,
            size: None,
        })
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(spawn_request_id))
        .await?;
    let response: ProcessSpawnResponse = to_response(response)?;
    assert_eq!(
        response,
        ProcessSpawnResponse {
            process_handle: process_handle.clone(),
        }
    );

    let exited = read_process_exited(&mut mcp).await?;
    assert_eq!(
        exited,
        ProcessExitedNotification {
            process_handle,
            exit_code: 0,
            stdout: "abc".to_string(),
            stdout_cap_reached: true,
            stderr: "123".to_string(),
            stderr_cap_reached: true,
        }
    );

    Ok(())
}

#[tokio::test]
async fn process_kill_terminates_running_process() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let process_handle = "sleep-process-1".to_string();
    let spawn_request_id = mcp
        .send_process_spawn_request(ProcessSpawnParams {
            command: vec!["sh".to_string(), "-lc".to_string(), "sleep 30".to_string()],
            process_handle: process_handle.clone(),
            cwd: AbsolutePathBuf::try_from(codex_home.path())?,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            timeout_ms: None,
            env: None,
            size: None,
        })
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(spawn_request_id))
        .await?;
    let response: ProcessSpawnResponse = to_response(response)?;
    assert_eq!(
        response,
        ProcessSpawnResponse {
            process_handle: process_handle.clone(),
        }
    );

    let kill_request_id = mcp
        .send_process_kill_request(ProcessKillParams {
            process_handle: process_handle.clone(),
        })
        .await?;
    let kill_response = mcp
        .read_stream_until_response_message(RequestId::Integer(kill_request_id))
        .await?;
    assert_eq!(kill_response.result, serde_json::json!({}));

    let exited = read_process_exited(&mut mcp).await?;
    assert_eq!(exited.process_handle, process_handle);
    assert_ne!(exited.exit_code, 0);
    assert_eq!(exited.stdout, "");
    assert!(!exited.stdout_cap_reached);
    assert_eq!(exited.stderr, "");
    assert!(!exited.stderr_cap_reached);

    Ok(())
}

async fn read_process_exited(mcp: &mut McpProcess) -> Result<ProcessExitedNotification> {
    let notification = mcp
        .read_stream_until_notification_message("process/exited")
        .await?;
    let params = notification
        .params
        .context("process/exited notification should include params")?;
    serde_json::from_value(params).context("deserialize process/exited notification")
}
