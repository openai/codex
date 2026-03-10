use anyhow::Context;
use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_protocol::CommandExecOutputDeltaNotification;
use codex_app_server_protocol::CommandExecOutputStream;
use codex_app_server_protocol::CommandExecParams;
use codex_app_server_protocol::CommandExecResizeParams;
use codex_app_server_protocol::CommandExecResponse;
use codex_app_server_protocol::CommandExecTerminalSize;
use codex_app_server_protocol::CommandExecTerminateParams;
use codex_app_server_protocol::CommandExecWriteParams;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;

use super::connection_handling_websocket::DEFAULT_READ_TIMEOUT;
use super::connection_handling_websocket::assert_no_message;
use super::connection_handling_websocket::connect_websocket;
use super::connection_handling_websocket::create_config_toml;
use super::connection_handling_websocket::read_jsonrpc_message;
use super::connection_handling_websocket::send_initialize_request;
use super::connection_handling_websocket::send_request;
use super::connection_handling_websocket::spawn_websocket_server;

#[tokio::test]
async fn command_exec_without_streams_can_be_terminated() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let process_id = "sleep-1".to_string();
    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec!["sh".to_string(), "-lc".to_string(), "sleep 30".to_string()],
            process_id: Some(process_id.clone()),
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
        })
        .await?;
    let terminate_request_id = mcp
        .send_command_exec_terminate_request(CommandExecTerminateParams { process_id })
        .await?;

    let terminate_response = mcp
        .read_stream_until_response_message(RequestId::Integer(terminate_request_id))
        .await?;
    assert_eq!(terminate_response.result, serde_json::json!({}));

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_ne!(
        response.exit_code, 0,
        "terminated command should not succeed"
    );
    assert_eq!(response.stdout, "");
    assert_eq!(response.stderr, "");

    Ok(())
}

#[tokio::test]
async fn command_exec_without_process_id_keeps_buffered_compatibility() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "printf 'legacy-out'; printf 'legacy-err' >&2".to_string(),
            ],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
        })
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(
        response,
        CommandExecResponse {
            exit_code: 0,
            stdout: "legacy-out".to_string(),
            stderr: "legacy-err".to_string(),
        }
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_env_overrides_merge_with_server_environment_and_support_unset() -> Result<()>
{
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[("COMMAND_EXEC_BASELINE", Some("server"))],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "/bin/sh".to_string(),
                "-lc".to_string(),
                "printf '%s|%s|%s|%s' \"$COMMAND_EXEC_BASELINE\" \"$COMMAND_EXEC_EXTRA\" \"${RUST_LOG-unset}\" \"$CODEX_HOME\"".to_string(),
            ],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: Some(HashMap::from([
                (
                    "COMMAND_EXEC_BASELINE".to_string(),
                    Some("request".to_string()),
                ),
                ("COMMAND_EXEC_EXTRA".to_string(), Some("added".to_string())),
                ("RUST_LOG".to_string(), None),
            ])),
            size: None,
            sandbox_policy: None,
        })
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(
        response,
        CommandExecResponse {
            exit_code: 0,
            stdout: format!("request|added|unset|{}", codex_home.path().display()),
            stderr: String::new(),
        }
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_rejects_disable_timeout_with_timeout_ms() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec!["sh".to_string(), "-lc".to_string(), "sleep 1".to_string()],
            process_id: Some("invalid-timeout-1".to_string()),
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: true,
            timeout_ms: Some(1_000),
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
        })
        .await?;

    let error = mcp
        .read_stream_until_error_message(RequestId::Integer(command_request_id))
        .await?;
    assert_eq!(
        error.error.message,
        "command/exec cannot set both timeoutMs and disableTimeout"
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_rejects_disable_output_cap_with_output_bytes_cap() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec!["sh".to_string(), "-lc".to_string(), "sleep 1".to_string()],
            process_id: Some("invalid-cap-1".to_string()),
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: Some(1024),
            disable_output_cap: true,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
        })
        .await?;

    let error = mcp
        .read_stream_until_error_message(RequestId::Integer(command_request_id))
        .await?;
    assert_eq!(
        error.error.message,
        "command/exec cannot set both outputBytesCap and disableOutputCap"
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_rejects_negative_timeout_ms() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec!["sh".to_string(), "-lc".to_string(), "sleep 1".to_string()],
            process_id: Some("negative-timeout-1".to_string()),
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: Some(-1),
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
        })
        .await?;

    let error = mcp
        .read_stream_until_error_message(RequestId::Integer(command_request_id))
        .await?;
    assert_eq!(
        error.error.message,
        "command/exec timeoutMs must be non-negative, got -1"
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_without_process_id_rejects_streaming() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec!["sh".to_string(), "-lc".to_string(), "cat".to_string()],
            process_id: None,
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: true,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
        })
        .await?;

    let error = mcp
        .read_stream_until_error_message(RequestId::Integer(command_request_id))
        .await?;
    assert_eq!(
        error.error.message,
        "command/exec tty or streaming requires a client-supplied processId"
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_non_streaming_respects_output_cap() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "printf 'abcdef'; printf 'uvwxyz' >&2".to_string(),
            ],
            process_id: Some("cap-1".to_string()),
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: Some(5),
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
        })
        .await?;

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(
        response,
        CommandExecResponse {
            exit_code: 0,
            stdout: "abcde".to_string(),
            stderr: "uvwxy".to_string(),
        }
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_streaming_does_not_buffer_output() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let process_id = "stream-cap-1".to_string();
    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "printf 'abcdefghij'; sleep 30".to_string(),
            ],
            process_id: Some(process_id.clone()),
            tty: false,
            stream_stdin: false,
            stream_stdout_stderr: true,
            output_bytes_cap: Some(5),
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
        })
        .await?;

    let delta = read_command_exec_delta(&mut mcp).await?;
    assert_eq!(delta.process_id, process_id.as_str());
    assert_eq!(delta.stream, CommandExecOutputStream::Stdout);
    assert_eq!(STANDARD.decode(&delta.delta_base64)?, b"abcde");
    assert!(delta.cap_reached);
    let terminate_request_id = mcp
        .send_command_exec_terminate_request(CommandExecTerminateParams {
            process_id: process_id.clone(),
        })
        .await?;
    let terminate_response = mcp
        .read_stream_until_response_message(RequestId::Integer(terminate_request_id))
        .await?;
    assert_eq!(terminate_response.result, serde_json::json!({}));

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_ne!(
        response.exit_code, 0,
        "terminated command should not succeed"
    );
    assert_eq!(response.stdout, "");
    assert_eq!(response.stderr, "");

    Ok(())
}

#[tokio::test]
async fn command_exec_pipe_streams_output_and_accepts_write() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let process_id = "pipe-1".to_string();
    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "printf 'out-start\\n'; printf 'err-start\\n' >&2; IFS= read line; printf 'out:%s\\n' \"$line\"; printf 'err:%s\\n' \"$line\" >&2".to_string(),
            ],
            process_id: Some(process_id.clone()),
            tty: false,
            stream_stdin: true,
            stream_stdout_stderr: true,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
        })
        .await?;

    let first_stdout = read_command_exec_delta(&mut mcp).await?;
    let first_stderr = read_command_exec_delta(&mut mcp).await?;
    let seen = [first_stdout, first_stderr];
    assert!(
        seen.iter()
            .all(|delta| delta.process_id == process_id.as_str())
    );
    assert!(seen.iter().any(|delta| {
        delta.stream == CommandExecOutputStream::Stdout
            && delta.delta_base64 == STANDARD.encode("out-start\n")
    }));
    assert!(seen.iter().any(|delta| {
        delta.stream == CommandExecOutputStream::Stderr
            && delta.delta_base64 == STANDARD.encode("err-start\n")
    }));

    let write_request_id = mcp
        .send_command_exec_write_request(CommandExecWriteParams {
            process_id: process_id.clone(),
            delta_base64: Some(STANDARD.encode("hello\n")),
            close_stdin: true,
        })
        .await?;
    let write_response = mcp
        .read_stream_until_response_message(RequestId::Integer(write_request_id))
        .await?;
    assert_eq!(write_response.result, serde_json::json!({}));

    let next_delta = read_command_exec_delta(&mut mcp).await?;
    let final_delta = read_command_exec_delta(&mut mcp).await?;
    let seen = [next_delta, final_delta];
    assert!(
        seen.iter()
            .all(|delta| delta.process_id == process_id.as_str())
    );
    assert!(seen.iter().any(|delta| {
        delta.stream == CommandExecOutputStream::Stdout
            && delta.delta_base64 == STANDARD.encode("out:hello\n")
    }));
    assert!(seen.iter().any(|delta| {
        delta.stream == CommandExecOutputStream::Stderr
            && delta.delta_base64 == STANDARD.encode("err:hello\n")
    }));

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(
        response,
        CommandExecResponse {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        }
    );

    Ok(())
}

#[tokio::test]
async fn command_exec_tty_implies_streaming_and_reports_pty_output() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let process_id = "tty-1".to_string();
    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "stty -echo; if [ -t 0 ]; then printf 'tty\\n'; else printf 'notty\\n'; fi; IFS= read line; printf 'echo:%s\\n' \"$line\"".to_string(),
            ],
            process_id: Some(process_id.clone()),
            tty: true,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: None,
            sandbox_policy: None,
        })
        .await?;

    let started_text = read_command_exec_output_until_contains(
        &mut mcp,
        process_id.as_str(),
        CommandExecOutputStream::Stdout,
        "tty\n",
    )
    .await?;
    assert!(
        started_text.contains("tty\n"),
        "expected TTY startup output, got {started_text:?}"
    );

    let write_request_id = mcp
        .send_command_exec_write_request(CommandExecWriteParams {
            process_id: process_id.clone(),
            delta_base64: Some(STANDARD.encode("world\n")),
            close_stdin: true,
        })
        .await?;
    let write_response = mcp
        .read_stream_until_response_message(RequestId::Integer(write_request_id))
        .await?;
    assert_eq!(write_response.result, serde_json::json!({}));

    let echoed_text = read_command_exec_output_until_contains(
        &mut mcp,
        process_id.as_str(),
        CommandExecOutputStream::Stdout,
        "echo:world\n",
    )
    .await?;
    assert!(
        echoed_text.contains("echo:world\n"),
        "expected TTY echo output, got {echoed_text:?}"
    );

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.stdout, "");
    assert_eq!(response.stderr, "");

    Ok(())
}

#[tokio::test]
async fn command_exec_tty_supports_initial_size_and_resize() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let process_id = "tty-size-1".to_string();
    let command_request_id = mcp
        .send_command_exec_request(CommandExecParams {
            command: vec![
                "sh".to_string(),
                "-lc".to_string(),
                "stty -echo; printf 'start:%s\\n' \"$(stty size)\"; IFS= read _line; printf 'after:%s\\n' \"$(stty size)\"".to_string(),
            ],
            process_id: Some(process_id.clone()),
            tty: true,
            stream_stdin: false,
            stream_stdout_stderr: false,
            output_bytes_cap: None,
            disable_output_cap: false,
            disable_timeout: false,
            timeout_ms: None,
            cwd: None,
            env: None,
            size: Some(CommandExecTerminalSize {
                rows: 31,
                cols: 101,
            }),
            sandbox_policy: None,
        })
        .await?;

    let started_text = read_command_exec_output_until_contains(
        &mut mcp,
        process_id.as_str(),
        CommandExecOutputStream::Stdout,
        "start:31 101\n",
    )
    .await?;
    assert!(
        started_text.contains("start:31 101\n"),
        "unexpected initial size output: {started_text:?}"
    );

    let resize_request_id = mcp
        .send_command_exec_resize_request(CommandExecResizeParams {
            process_id: process_id.clone(),
            size: CommandExecTerminalSize {
                rows: 45,
                cols: 132,
            },
        })
        .await?;
    let resize_response = mcp
        .read_stream_until_response_message(RequestId::Integer(resize_request_id))
        .await?;
    assert_eq!(resize_response.result, serde_json::json!({}));

    let write_request_id = mcp
        .send_command_exec_write_request(CommandExecWriteParams {
            process_id: process_id.clone(),
            delta_base64: Some(STANDARD.encode("go\n")),
            close_stdin: true,
        })
        .await?;
    let write_response = mcp
        .read_stream_until_response_message(RequestId::Integer(write_request_id))
        .await?;
    assert_eq!(write_response.result, serde_json::json!({}));

    let resized_text = read_command_exec_output_until_contains(
        &mut mcp,
        process_id.as_str(),
        CommandExecOutputStream::Stdout,
        "after:45 132\n",
    )
    .await?;
    assert!(
        resized_text.contains("after:45 132\n"),
        "unexpected resized output: {resized_text:?}"
    );

    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(command_request_id))
        .await?;
    let response: CommandExecResponse = to_response(response)?;
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.stdout, "");
    assert_eq!(response.stderr, "");

    Ok(())
}

#[tokio::test]
async fn command_exec_process_ids_are_connection_scoped_and_disconnect_terminates_process()
-> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let marker = format!(
        "codex-command-exec-marker-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );

    let (mut process, bind_addr) = spawn_websocket_server(codex_home.path()).await?;

    let mut ws1 = connect_websocket(bind_addr).await?;
    let mut ws2 = connect_websocket(bind_addr).await?;

    send_initialize_request(&mut ws1, 1, "ws_client_one").await?;
    read_initialize_response(&mut ws1, 1).await?;
    send_initialize_request(&mut ws2, 2, "ws_client_two").await?;
    read_initialize_response(&mut ws2, 2).await?;

    send_request(
        &mut ws1,
        "command/exec",
        101,
        Some(serde_json::json!({
            "command": [
                "python3",
                "-c",
                "import time; print('ready', flush=True); time.sleep(30)",
                marker,
            ],
            "processId": "shared-process",
            "streamStdoutStderr": true,
        })),
    )
    .await?;

    let delta = read_command_exec_delta_ws(&mut ws1).await?;
    assert_eq!(delta.process_id, "shared-process");
    assert_eq!(delta.stream, CommandExecOutputStream::Stdout);
    let delta_text = String::from_utf8(STANDARD.decode(&delta.delta_base64)?)?;
    assert!(delta_text.contains("ready"));
    wait_for_process_marker(&marker, true).await?;

    send_request(
        &mut ws2,
        "command/exec/terminate",
        102,
        Some(serde_json::json!({
            "processId": "shared-process",
        })),
    )
    .await?;

    let terminate_error = loop {
        let message = read_jsonrpc_message(&mut ws2).await?;
        if let JSONRPCMessage::Error(error) = message
            && error.id == RequestId::Integer(102)
        {
            break error;
        }
    };
    assert_eq!(
        terminate_error.error.message,
        "no active command/exec for process id \"shared-process\""
    );
    wait_for_process_marker(&marker, true).await?;

    assert_no_message(&mut ws2, Duration::from_millis(250)).await?;
    ws1.close(None).await?;

    wait_for_process_marker(&marker, false).await?;

    process
        .kill()
        .await
        .context("failed to stop websocket app-server process")?;
    Ok(())
}

async fn read_command_exec_delta(
    mcp: &mut McpProcess,
) -> Result<CommandExecOutputDeltaNotification> {
    let notification = mcp
        .read_stream_until_notification_message("command/exec/outputDelta")
        .await?;
    decode_delta_notification(notification)
}

async fn read_command_exec_output_until_contains(
    mcp: &mut McpProcess,
    process_id: &str,
    stream: CommandExecOutputStream,
    expected: &str,
) -> Result<String> {
    let deadline = Instant::now() + DEFAULT_READ_TIMEOUT;
    let mut collected = String::new();

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let delta = timeout(remaining, read_command_exec_delta(mcp))
            .await
            .with_context(|| {
                format!(
                    "timed out waiting for {expected:?} in command/exec output for {process_id}; collected {collected:?}"
                )
            })??;
        assert_eq!(delta.process_id, process_id);
        assert_eq!(delta.stream, stream);

        let delta_text = String::from_utf8(STANDARD.decode(&delta.delta_base64)?)?;
        collected.push_str(&delta_text.replace('\r', ""));
        if collected.contains(expected) {
            return Ok(collected);
        }
    }
}

async fn read_command_exec_delta_ws(
    stream: &mut super::connection_handling_websocket::WsClient,
) -> Result<CommandExecOutputDeltaNotification> {
    loop {
        let message = read_jsonrpc_message(stream).await?;
        let JSONRPCMessage::Notification(notification) = message else {
            continue;
        };
        if notification.method == "command/exec/outputDelta" {
            return decode_delta_notification(notification);
        }
    }
}

fn decode_delta_notification(
    notification: JSONRPCNotification,
) -> Result<CommandExecOutputDeltaNotification> {
    let params = notification
        .params
        .context("command/exec/outputDelta notification should include params")?;
    serde_json::from_value(params).context("deserialize command/exec/outputDelta notification")
}

async fn read_initialize_response(
    stream: &mut super::connection_handling_websocket::WsClient,
    request_id: i64,
) -> Result<()> {
    loop {
        let message = read_jsonrpc_message(stream).await?;
        if let JSONRPCMessage::Response(response) = message
            && response.id == RequestId::Integer(request_id)
        {
            return Ok(());
        }
    }
}

async fn wait_for_process_marker(marker: &str, should_exist: bool) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if process_with_marker_exists(marker)? == should_exist {
            return Ok(());
        }
        if Instant::now() >= deadline {
            let expectation = if should_exist { "appear" } else { "exit" };
            anyhow::bail!("process marker {marker:?} did not {expectation} before timeout");
        }
        sleep(Duration::from_millis(50)).await;
    }
}

fn process_with_marker_exists(marker: &str) -> Result<bool> {
    let output = std::process::Command::new("ps")
        .args(["-axo", "command"])
        .output()
        .context("spawn ps -axo command")?;
    let stdout = String::from_utf8(output.stdout).context("decode ps output")?;
    Ok(stdout.lines().any(|line| line.contains(marker)))
}
