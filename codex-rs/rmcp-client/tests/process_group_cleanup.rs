#![cfg(unix)]

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_rmcp_client::RmcpClient;

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
                let pid = content
                    .trim()
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
            "CHILD_PID_FILE".to_string(),
            child_pid_file_str,
        )])),
        &[],
        None,
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
