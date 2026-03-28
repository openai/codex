use super::connection_handling_websocket::DEFAULT_READ_TIMEOUT;
use super::connection_handling_websocket::WsClient;
use super::connection_handling_websocket::connect_websocket;
use super::connection_handling_websocket::create_config_toml;
use super::connection_handling_websocket::read_response_for_id;
use super::connection_handling_websocket::send_initialize_request;
use super::connection_handling_websocket::send_request;
use super::connection_handling_websocket::spawn_websocket_server;
use anyhow::Context;
use anyhow::Result;
use app_test_support::create_fake_rollout_with_text_elements;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use core_test_support::stdio_server_bin;
use pretty_assertions::assert_eq;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::sleep;
use tokio::time::timeout;

fn process_is_running(pid: u32) -> bool {
    let output = Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .output();

    output
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .find_map(|line| {
                    let trimmed = line.trim();
                    (!trimmed.is_empty()).then_some(trimmed.to_owned())
                })
        })
        .is_some_and(|stat| !stat.starts_with('Z'))
}

async fn wait_for_pid_entry(path: &Path) -> Result<u32> {
    for _ in 0..50 {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let Some(pid) = content
                    .lines()
                    .find_map(|line| {
                        let trimmed = line.trim();
                        (!trimmed.is_empty()).then_some(trimmed)
                    })
                    .map(str::parse::<u32>)
                    .transpose()
                    .with_context(|| format!("failed to parse pid from {}", path.display()))?
                else {
                    sleep(Duration::from_millis(100)).await;
                    continue;
                };
                return Ok(pid);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                sleep(Duration::from_millis(100)).await;
            }
            Err(error) => {
                return Err(error).with_context(|| format!("failed to read {}", path.display()));
            }
        }
    }

    anyhow::bail!("timed out waiting for pid file at {}", path.display());
}

async fn wait_for_process_exit(pid: u32) -> Result<()> {
    for _ in 0..50 {
        if !process_is_running(pid) {
            return Ok(());
        }
        sleep(Duration::from_millis(100)).await;
    }

    anyhow::bail!("process {pid} still running after timeout");
}

fn ensure_stdio_server_bin() -> Result<String> {
    match stdio_server_bin() {
        Ok(bin) => Ok(bin),
        Err(_) => {
            let status = Command::new("cargo")
                .args([
                    "build",
                    "-p",
                    "codex-rmcp-client",
                    "--bin",
                    "test_stdio_server",
                ])
                .status()
                .context("failed to invoke cargo to build test_stdio_server")?;
            anyhow::ensure!(
                status.success(),
                "cargo build -p codex-rmcp-client --bin test_stdio_server failed with status {status}"
            );
            stdio_server_bin()
                .context("test_stdio_server binary still unavailable after cargo build")
        }
    }
}

fn create_config_toml_with_local_mcp(
    codex_home: &Path,
    server_uri: &str,
    wrapper_path: &Path,
) -> std::io::Result<()> {
    create_config_toml(codex_home, server_uri, "never")?;
    let config_toml = codex_home.join("config.toml");
    let existing = std::fs::read_to_string(&config_toml)?;
    std::fs::write(
        &config_toml,
        format!(
            r#"{existing}

[mcp_servers.test_stdio]
command = "{}"
"#,
            wrapper_path.display(),
        ),
    )
}

fn create_pid_logging_stdio_wrapper(
    dir: &Path,
    pid_file: &Path,
    server_bin: &str,
) -> std::io::Result<std::path::PathBuf> {
    let wrapper_path = dir.join("mcp-wrapper.sh");
    std::fs::write(
        &wrapper_path,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$$\" >> '{}'\nexec '{}' \"$@\"\n",
            pid_file.display(),
            server_bin,
        ),
    )?;
    let mut permissions = std::fs::metadata(&wrapper_path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&wrapper_path, permissions)?;
    Ok(wrapper_path)
}

fn create_rollout(codex_home: &Path, filename_ts: &str) -> Result<String> {
    create_fake_rollout_with_text_elements(
        codex_home,
        filename_ts,
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Vec::new(),
        Some("mock_provider"),
        None,
    )
}

async fn initialize_ws_client(bind_addr: std::net::SocketAddr) -> Result<WsClient> {
    let mut ws = connect_websocket(bind_addr).await?;
    send_initialize_request(&mut ws, 1, "ws_disconnect_client").await?;
    timeout(DEFAULT_READ_TIMEOUT, read_response_for_id(&mut ws, 1)).await??;
    Ok(ws)
}

#[tokio::test]
async fn websocket_disconnect_unloads_resumed_thread_and_reaps_stdio_mcp() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    let pid_file = codex_home.path().join("mcp-pids.log");
    let stdio_server = ensure_stdio_server_bin()?;
    let wrapper_path =
        create_pid_logging_stdio_wrapper(codex_home.path(), &pid_file, &stdio_server)?;
    create_config_toml_with_local_mcp(codex_home.path(), &server.uri(), &wrapper_path)?;
    let conversation_id = create_rollout(codex_home.path(), "2025-01-05T12-00-00")?;

    let (mut process, bind_addr) = spawn_websocket_server(codex_home.path()).await?;

    let result = async {
        let mut ws = initialize_ws_client(bind_addr).await?;
        send_request(
            &mut ws,
            "thread/resume",
            10,
            Some(serde_json::to_value(ThreadResumeParams {
                thread_id: conversation_id.clone(),
                ..Default::default()
            })?),
        )
        .await?;
        let resume_resp: JSONRPCResponse =
            timeout(DEFAULT_READ_TIMEOUT, read_response_for_id(&mut ws, 10)).await??;
        let resume: ThreadResumeResponse = to_response::<ThreadResumeResponse>(resume_resp)?;
        assert_eq!(resume.thread.id, conversation_id);

        let pid = wait_for_pid_entry(&pid_file).await?;
        assert!(
            process_is_running(pid),
            "expected stdio MCP process {pid} to be running before disconnect"
        );

        ws.close(None)
            .await
            .context("failed to close websocket connection")?;
        drop(ws);

        wait_for_process_exit(pid).await?;

        let mut ws = connect_websocket(bind_addr).await?;
        send_initialize_request(&mut ws, 2, "ws_disconnect_client_reconnect").await?;
        timeout(DEFAULT_READ_TIMEOUT, read_response_for_id(&mut ws, 2)).await??;

        send_request(
            &mut ws,
            "thread/loaded/list",
            11,
            Some(serde_json::to_value(ThreadLoadedListParams::default())?),
        )
        .await?;
        let list_resp: JSONRPCResponse =
            timeout(DEFAULT_READ_TIMEOUT, read_response_for_id(&mut ws, 11)).await??;
        let ThreadLoadedListResponse { data, next_cursor } =
            to_response::<ThreadLoadedListResponse>(list_resp)?;
        assert_eq!(data, Vec::<String>::new());
        assert_eq!(next_cursor, None);

        Ok(())
    }
    .await;

    process
        .kill()
        .await
        .context("failed to stop websocket app-server process")?;
    result
}
