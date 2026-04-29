#![cfg(unix)]

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::LocalStdioServerLauncher;
use codex_rmcp_client::RmcpClient;
use futures::FutureExt as _;
use rmcp::model::ClientCapabilities;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ProtocolVersion;
use serde_json::json;

fn stdio_server_bin() -> Result<std::path::PathBuf> {
    codex_utils_cargo_bin::cargo_bin("test_stdio_server").map_err(Into::into)
}

fn init_params() -> InitializeRequestParams {
    InitializeRequestParams {
        meta: None,
        capabilities: ClientCapabilities {
            experimental: None,
            extensions: None,
            roots: None,
            sampling: None,
            elicitation: None,
            tasks: None,
        },
        client_info: Implementation {
            name: "codex-test".into(),
            version: "0.0.0-test".into(),
            title: Some("Codex rmcp shutdown test".into()),
            description: None,
            icons: None,
            website_url: None,
        },
        protocol_version: ProtocolVersion::V_2025_06_18,
    }
}

fn process_exists(pid: u32) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

async fn wait_for_pid_file(path: &Path) -> Result<u32> {
    for _ in 0..50 {
        match fs::read_to_string(path) {
            Ok(content) => {
                let trimmed = content.trim();
                if trimmed.is_empty() {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }

                let pid = trimmed
                    .parse::<u32>()
                    .with_context(|| format!("failed to parse pid from {}", path.display()))?;
                return Ok(pid);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(error) => {
                return Err(error).with_context(|| format!("failed to read {}", path.display()));
            }
        }
    }

    anyhow::bail!("timed out waiting for child pid file at {}", path.display());
}

async fn wait_for_process_exit(pid: u32) -> Result<()> {
    for _ in 0..50 {
        if !process_exists(pid) {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    anyhow::bail!("process {pid} still running after timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn drop_kills_wrapper_process_group() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let child_pid_file = temp_dir.path().join("child.pid");
    let child_pid_file_str = child_pid_file.to_string_lossy().into_owned();

    let client = RmcpClient::new_stdio_client(
        OsString::from("/bin/sh"),
        vec![
            OsString::from("-c"),
            OsString::from(
                "sleep 300 & child_pid=$!; echo \"$child_pid\" > \"$CHILD_PID_FILE\"; cat >/dev/null",
            ),
        ],
        Some(HashMap::from([(
            OsString::from("CHILD_PID_FILE"),
            OsString::from(child_pid_file_str),
        )])),
        &[],
        /*cwd*/ None,
        Arc::new(LocalStdioServerLauncher::new(std::env::current_dir()?)),
    )
    .await?;

    let grandchild_pid = wait_for_pid_file(&child_pid_file).await?;
    assert!(
        process_exists(grandchild_pid),
        "expected grandchild process {grandchild_pid} to be running before dropping client"
    );

    drop(client);

    wait_for_process_exit(grandchild_pid).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_kills_initialized_stdio_server_with_in_flight_operation() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let server_pid_file = temp_dir.path().join("server.pid");
    let server_pid_file_str = server_pid_file.to_string_lossy().into_owned();

    let client = Arc::new(
        RmcpClient::new_stdio_client(
            stdio_server_bin()?.into(),
            Vec::<OsString>::new(),
            Some(HashMap::from([(
                OsString::from("MCP_TEST_PID_FILE"),
                OsString::from(server_pid_file_str),
            )])),
            &[],
            /*cwd*/ None,
            Arc::new(LocalStdioServerLauncher::new(std::env::current_dir()?)),
        )
        .await?,
    );

    client
        .initialize(
            init_params(),
            Some(Duration::from_secs(5)),
            Box::new(|_, _| {
                async {
                    Ok(ElicitationResponse {
                        action: ElicitationAction::Accept,
                        content: Some(json!({})),
                        meta: None,
                    })
                }
                .boxed()
            }),
        )
        .await?;

    let server_pid = wait_for_pid_file(&server_pid_file).await?;
    assert!(
        process_exists(server_pid),
        "expected MCP server process {server_pid} to be running before shutdown"
    );

    let call_client = Arc::clone(&client);
    let call_task = tokio::spawn(async move {
        call_client
            .call_tool(
                "sync".to_string(),
                Some(json!({ "sleep_after_ms": 300_000 })),
                /*meta*/ None,
                Some(Duration::from_secs(300)),
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(200)).await;

    client.shutdown().await;

    wait_for_process_exit(server_pid).await?;
    let _ = tokio::time::timeout(Duration::from_secs(5), call_task).await?;
    Ok(())
}
