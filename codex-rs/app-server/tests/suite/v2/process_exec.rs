use anyhow::Context;
use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use codex_app_server_protocol::ProcessExitedNotification;
use codex_app_server_protocol::ProcessKillParams;
use codex_app_server_protocol::ProcessSpawnParams;
use codex_app_server_protocol::RequestId;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::sleep;
use tokio::time::timeout;
use wiremock::MockServer;

use super::connection_handling_websocket::DEFAULT_READ_TIMEOUT;
use super::connection_handling_websocket::create_config_toml;

#[tokio::test]
async fn process_spawn_returns_before_exit_and_emits_exit_notification() -> Result<()> {
    let codex_home = TempDir::new()?;
    let (_server, mut mcp) = initialized_mcp(codex_home.path()).await?;

    let process_handle = "one-shot-1".to_string();
    let probe_file = codex_home.path().join("process-created");
    let release_file = codex_home.path().join("process-release");
    // Use a probe/release handshake instead of asserting on wall-clock timing:
    // the child proves it started by writing the probe file, then waits for the
    // test to create the release file before it can emit output and exit.
    let command = if cfg!(windows) {
        vec![
            "powershell.exe".to_string(),
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-Command".to_string(),
            concat!(
                "[IO.File]::WriteAllText($env:CODEX_PROCESS_EXEC_PROBE_FILE, 'process'); ",
                "while (!(Test-Path -LiteralPath $env:CODEX_PROCESS_EXEC_RELEASE_FILE)) { ",
                "Start-Sleep -Milliseconds 20 ",
                "}; ",
                "[Console]::Out.Write('process-out'); ",
                "[Console]::Error.Write('process-err')",
            )
            .to_string(),
        ]
    } else {
        vec![
            "sh".to_string(),
            "-c".to_string(),
            concat!(
                "printf process > \"$CODEX_PROCESS_EXEC_PROBE_FILE\"; ",
                "while [ ! -e \"$CODEX_PROCESS_EXEC_RELEASE_FILE\" ]; do sleep 0.05; done; ",
                "printf process-out; ",
                "printf process-err >&2",
            )
            .to_string(),
        ]
    };
    let env = HashMap::from([
        (
            "CODEX_PROCESS_EXEC_PROBE_FILE".to_string(),
            Some(probe_file.display().to_string()),
        ),
        (
            "CODEX_PROCESS_EXEC_RELEASE_FILE".to_string(),
            Some(release_file.display().to_string()),
        ),
    ]);
    let spawn_request_id = mcp
        .send_process_spawn_request(ProcessSpawnParams {
            env: Some(env),
            output_bytes_cap: Some(None),
            timeout_ms: Some(None),
            ..process_spawn_params(process_handle.clone(), codex_home.path(), command)?
        })
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(spawn_request_id))
        .await?;
    assert_eq!(response.result, serde_json::json!({}));

    wait_for_file(&probe_file).await?;
    assert_eq!(std::fs::read_to_string(&probe_file)?, "process");
    std::fs::write(&release_file, "release")?;

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
    Ok(())
}

#[tokio::test]
async fn process_spawn_reports_buffered_output_cap_reached() -> Result<()> {
    let codex_home = TempDir::new()?;
    let (_server, mut mcp) = initialized_mcp(codex_home.path()).await?;

    let process_handle = "capped-one-shot-1".to_string();
    let command = if cfg!(windows) {
        vec![
            "powershell.exe".to_string(),
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-Command".to_string(),
            "[Console]::Out.Write('abcde'); [Console]::Error.Write('12345')".to_string(),
        ]
    } else {
        vec![
            "sh".to_string(),
            "-lc".to_string(),
            "printf abcde; printf 12345 >&2".to_string(),
        ]
    };
    let spawn_request_id = mcp
        .send_process_spawn_request(ProcessSpawnParams {
            output_bytes_cap: Some(Some(3)),
            ..process_spawn_params(process_handle.clone(), codex_home.path(), command)?
        })
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(spawn_request_id))
        .await?;
    assert_eq!(response.result, serde_json::json!({}));

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
    let codex_home = TempDir::new()?;
    let (_server, mut mcp) = initialized_mcp(codex_home.path()).await?;

    let process_handle = "sleep-process-1".to_string();
    let command = if cfg!(windows) {
        vec![
            "powershell.exe".to_string(),
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-Command".to_string(),
            "Start-Sleep -Seconds 30".to_string(),
        ]
    } else {
        vec!["sh".to_string(), "-lc".to_string(), "sleep 30".to_string()]
    };
    let spawn_request_id = mcp
        .send_process_spawn_request(process_spawn_params(
            process_handle.clone(),
            codex_home.path(),
            command,
        )?)
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(spawn_request_id))
        .await?;
    assert_eq!(response.result, serde_json::json!({}));

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

async fn initialized_mcp(codex_home: &Path) -> Result<(MockServer, McpProcess)> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    create_config_toml(codex_home, &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    Ok((server, mcp))
}

fn process_spawn_params(
    process_handle: String,
    cwd: &Path,
    command: Vec<String>,
) -> Result<ProcessSpawnParams> {
    Ok(ProcessSpawnParams {
        command,
        process_handle,
        cwd: AbsolutePathBuf::try_from(cwd)?,
        tty: false,
        stream_stdin: false,
        stream_stdout_stderr: false,
        output_bytes_cap: None,
        timeout_ms: None,
        env: None,
        size: None,
    })
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

async fn wait_for_file(path: &Path) -> Result<()> {
    timeout(DEFAULT_READ_TIMEOUT, async {
        while !path.exists() {
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .context("timed out waiting for process probe file")
}
