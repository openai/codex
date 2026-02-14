#![cfg(unix)]

use anyhow::Context;
use anyhow::Result;
use serde_json::Value as JsonValue;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::time::Duration;
use tokio::time::timeout;

const JSONRPC_VERSION: &str = "2.0";

#[tokio::test]
async fn exec_start_emits_multiple_subcommand_approvals_for_compound_command() -> Result<()> {
    let Some(zsh_path) = std::env::var_os("CODEX_TEST_ZSH_PATH") else {
        eprintln!("skipping direct sidecar protocol test: CODEX_TEST_ZSH_PATH is not set");
        return Ok(());
    };
    let zsh_path = std::path::PathBuf::from(zsh_path);
    if !zsh_path.is_file() {
        anyhow::bail!(
            "CODEX_TEST_ZSH_PATH is set but is not a file: {}",
            zsh_path.display()
        );
    }

    let sidecar = env!("CARGO_BIN_EXE_codex-zsh-sidecar");
    let mut child = Command::new(sidecar)
        .arg("--zsh-path")
        .arg(&zsh_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .context("spawn codex-zsh-sidecar")?;

    let mut stdin = child.stdin.take().context("missing sidecar stdin")?;
    let stdout = child.stdout.take().context("missing sidecar stdout")?;
    let mut lines = BufReader::new(stdout).lines();

    write_json_line(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": 1,
            "method": "zsh/initialize",
            "params": {
                "sessionId": "test-session"
            }
        }),
    )
    .await?;
    wait_for_response(&mut lines, 1).await?;

    write_json_line(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": 2,
            "method": "zsh/execStart",
            "params": {
                "execId": "exec-test-1",
                "command": [zsh_path.to_string_lossy(), "-fc", "/usr/bin/true && /usr/bin/true"],
                "cwd": std::env::current_dir()?.to_string_lossy().to_string(),
                "env": {}
            }
        }),
    )
    .await?;

    let mut exec_start_acked = false;
    let mut intercepted_subcommand_callbacks = 0usize;
    let mut intercepted_true_callbacks = 0usize;
    let mut saw_exec_exited = false;
    let mut exit_code = None;

    while !saw_exec_exited {
        let line = timeout(Duration::from_secs(10), lines.next_line())
            .await
            .context("timed out reading sidecar output")??
            .context("sidecar stdout closed unexpectedly")?;
        let value: JsonValue = serde_json::from_str(&line).context("parse sidecar JSON line")?;

        if value.get("method").and_then(JsonValue::as_str) == Some("zsh/requestApproval") {
            let id = value
                .get("id")
                .cloned()
                .context("approval request missing id")?;
            let reason = value
                .pointer("/params/reason")
                .and_then(JsonValue::as_str)
                .unwrap_or_default();
            let command = value
                .pointer("/params/command")
                .and_then(JsonValue::as_array)
                .cloned()
                .unwrap_or_default();

            if reason == "zsh sidecar intercepted subcommand execve" {
                intercepted_subcommand_callbacks += 1;
                if command.first().and_then(JsonValue::as_str) == Some("/usr/bin/true") {
                    intercepted_true_callbacks += 1;
                }
            }

            write_json_line(
                &mut stdin,
                &serde_json::json!({
                    "jsonrpc": JSONRPC_VERSION,
                    "id": id,
                    "result": {
                        "decision": "approved"
                    }
                }),
            )
            .await?;
            continue;
        }

        if value.get("id").and_then(JsonValue::as_i64) == Some(2) && value.get("result").is_some() {
            exec_start_acked = true;
            continue;
        }

        if value.get("method").and_then(JsonValue::as_str) == Some("zsh/event/execExited") {
            saw_exec_exited = true;
            exit_code = value
                .pointer("/params/exitCode")
                .and_then(JsonValue::as_i64)
                .map(|code| code as i32);
        }
    }

    write_json_line(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": 3,
            "method": "zsh/shutdown",
            "params": {
                "graceMs": 100
            }
        }),
    )
    .await?;
    wait_for_response(&mut lines, 3).await?;

    let status = timeout(Duration::from_secs(3), child.wait())
        .await
        .context("timed out waiting for sidecar process exit")??;

    assert!(status.success(), "sidecar should exit cleanly");
    assert!(exec_start_acked, "expected execStart success response");
    assert_eq!(exit_code, Some(0), "expected successful command exit");
    assert!(
        intercepted_subcommand_callbacks >= 2,
        "expected at least two intercepted subcommand approvals, got {intercepted_subcommand_callbacks}"
    );
    assert!(
        intercepted_true_callbacks >= 2,
        "expected at least two intercepted /usr/bin/true approvals, got {intercepted_true_callbacks}"
    );

    Ok(())
}

async fn wait_for_response(
    lines: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    id: i64,
) -> Result<JsonValue> {
    loop {
        let line = timeout(Duration::from_secs(10), lines.next_line())
            .await
            .context("timed out waiting for response")??
            .context("sidecar stdout closed while waiting for response")?;
        let value: JsonValue = serde_json::from_str(&line).context("parse sidecar JSON line")?;
        if value.get("id").and_then(JsonValue::as_i64) == Some(id) {
            return Ok(value);
        }
    }
}

async fn write_json_line(stdin: &mut tokio::process::ChildStdin, value: &JsonValue) -> Result<()> {
    let encoded = serde_json::to_string(value).context("serialize JSON line")?;
    stdin
        .write_all(encoded.as_bytes())
        .await
        .context("write JSON line")?;
    stdin.write_all(b"\n").await.context("write line break")?;
    stdin.flush().await.context("flush stdin")?;
    Ok(())
}
