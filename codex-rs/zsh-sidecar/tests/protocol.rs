#![cfg(unix)]

use anyhow::Context;
use anyhow::Result;
use serde_json::Value as JsonValue;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::UnixListener;
use tokio::process::Child;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;
use tokio::process::Command;
use tokio::time::Duration;
use tokio::time::timeout;

const JSONRPC_VERSION: &str = "2.0";
const EXEC_START_REQUEST_ID: i64 = 2;
const WRAPPER_MODE_ENV_VAR: &str = "CODEX_ZSH_SIDECAR_WRAPPER_MODE";
const WRAPPER_SOCKET_ENV_VAR: &str = "CODEX_ZSH_SIDECAR_WRAPPER_SOCKET";
const WRAPPER_ORIGIN_ENV_VAR: &str = "CODEX_ZSH_SIDECAR_WRAPPER_ORIGIN";

#[tokio::test]
async fn exec_start_emits_multiple_subcommand_approvals_for_compound_command() -> Result<()> {
    let Some(mut harness) = SidecarHarness::start().await? else {
        return Ok(());
    };

    harness.initialize().await?;
    harness
        .start_exec_with_command("/usr/bin/true && /usr/bin/true", None)
        .await?;

    let mut exec_start_acked = false;
    let mut intercepted_subcommand_callbacks = 0usize;
    let mut intercepted_true_callbacks = 0usize;
    let mut saw_exec_exited = false;
    let mut exit_code = None;

    while !saw_exec_exited {
        let value = harness.read_next_message().await?;

        if let Some((id, _reason, command)) = parse_approval_request(&value) {
            if command.first().is_some_and(|c| c == "/usr/bin/true") {
                intercepted_subcommand_callbacks += 1;
                intercepted_true_callbacks += 1;
            }
            harness.respond_approval(id, "approved").await?;
            continue;
        }

        if value.get("id").and_then(JsonValue::as_i64) == Some(EXEC_START_REQUEST_ID)
            && value.get("result").is_some()
        {
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

    harness.shutdown().await?;

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

#[tokio::test]
async fn exec_start_uses_custom_approval_reason_when_provided() -> Result<()> {
    let Some(mut harness) = SidecarHarness::start().await? else {
        return Ok(());
    };

    harness.initialize().await?;
    harness
        .start_exec_with_command(
            "/usr/bin/true && /usr/bin/true",
            Some("test custom zsh approval reason"),
        )
        .await?;

    let mut saw_custom_reason = false;
    let mut saw_exec_exited = false;

    while !saw_exec_exited {
        let value = harness.read_next_message().await?;
        if let Some((id, reason, _command)) = parse_approval_request(&value) {
            if reason.as_deref() == Some("test custom zsh approval reason") {
                saw_custom_reason = true;
            }
            harness.respond_approval(id, "approved").await?;
            continue;
        }

        if value.get("method").and_then(JsonValue::as_str) == Some("zsh/event/execExited") {
            saw_exec_exited = true;
        }
    }

    harness.shutdown().await?;
    assert!(
        saw_custom_reason,
        "expected at least one approval callback with custom reason"
    );
    Ok(())
}

#[tokio::test]
async fn exec_start_does_not_emit_exec_start_approval_callback() -> Result<()> {
    let Some(mut harness) = SidecarHarness::start().await? else {
        return Ok(());
    };

    harness.initialize().await?;
    harness
        .start_exec_with_command("/usr/bin/true && /usr/bin/true", None)
        .await?;

    let mut saw_exec_start_success = false;
    let mut saw_exec_exited = false;
    let mut saw_exec_start_reason = false;

    while !saw_exec_exited {
        let value = harness.read_next_message().await?;
        if let Some((id, reason, _command)) = parse_approval_request(&value) {
            if reason.as_deref() == Some("zsh sidecar execStart command approval") {
                saw_exec_start_reason = true;
            }
            harness.respond_approval(id, "approved").await?;
            continue;
        }

        if value.get("id").and_then(JsonValue::as_i64) == Some(EXEC_START_REQUEST_ID)
            && value.get("result").is_some()
        {
            saw_exec_start_success = true;
            continue;
        }

        if value.get("method").and_then(JsonValue::as_str) == Some("zsh/event/execExited") {
            saw_exec_exited = true;
        }
    }

    harness.shutdown().await?;
    assert!(
        saw_exec_start_success,
        "expected execStart success response"
    );
    assert!(
        !saw_exec_start_reason,
        "did not expect execStart approval callback reason"
    );
    Ok(())
}

#[tokio::test]
async fn exec_start_does_not_emit_internal_zsh_reexec_approval_callback() -> Result<()> {
    let Some(mut harness) = SidecarHarness::start().await? else {
        return Ok(());
    };

    harness.initialize().await?;
    harness
        .start_exec_with_command("/usr/bin/true && /usr/bin/true", None)
        .await?;

    let mut saw_exec_start_success = false;
    let mut saw_exec_exited = false;
    let mut saw_internal_zsh_reexec = false;

    while !saw_exec_exited {
        let value = harness.read_next_message().await?;
        if let Some((id, _reason, command)) = parse_approval_request(&value) {
            let command_program_name = command
                .first()
                .map(std::path::PathBuf::from)
                .and_then(|path| path.file_name().map(std::borrow::ToOwned::to_owned))
                .and_then(|name| name.to_str().map(str::to_owned))
                .unwrap_or_default();
            let has_command_string_flag = command
                .iter()
                .filter(|arg| arg.starts_with('-'))
                .any(|arg| arg.chars().skip(1).any(|ch| ch == 'c'));
            if command_program_name == "zsh" && has_command_string_flag {
                saw_internal_zsh_reexec = true;
            }
            harness.respond_approval(id, "approved").await?;
            continue;
        }

        if value.get("id").and_then(JsonValue::as_i64) == Some(EXEC_START_REQUEST_ID)
            && value.get("result").is_some()
        {
            saw_exec_start_success = true;
            continue;
        }

        if value.get("method").and_then(JsonValue::as_str) == Some("zsh/event/execExited") {
            saw_exec_exited = true;
        }
    }

    harness.shutdown().await?;
    assert!(
        saw_exec_start_success,
        "expected execStart success response"
    );
    assert!(
        !saw_internal_zsh_reexec,
        "did not expect host approval callback for internal zsh re-exec"
    );
    Ok(())
}

#[tokio::test]
async fn denying_second_subcommand_fails_exec() -> Result<()> {
    let Some(outcome) = run_compound_exec_with_second_subcommand_decision("denied").await? else {
        return Ok(());
    };
    assert!(
        outcome.saw_exec_start_success,
        "expected execStart success response"
    );
    assert!(
        outcome.subcommand_callbacks >= 2,
        "expected at least two subcommand callbacks before exit, got {}",
        outcome.subcommand_callbacks
    );
    assert_ne!(
        outcome.exit_code,
        Some(0),
        "denying the second subcommand should cause non-zero exit"
    );
    Ok(())
}

#[tokio::test]
async fn aborting_second_subcommand_fails_exec() -> Result<()> {
    let Some(outcome) = run_compound_exec_with_second_subcommand_decision("abort").await? else {
        return Ok(());
    };
    assert!(
        outcome.saw_exec_start_success,
        "expected execStart success response"
    );
    assert!(
        outcome.subcommand_callbacks >= 2,
        "expected at least two subcommand callbacks before exit, got {}",
        outcome.subcommand_callbacks
    );
    assert_ne!(
        outcome.exit_code,
        Some(0),
        "aborting the second subcommand should cause non-zero exit"
    );
    Ok(())
}

#[tokio::test]
async fn approved_for_session_subcommand_decision_allows_exec() -> Result<()> {
    let Some(outcome) =
        run_compound_exec_with_second_subcommand_decision("approved_for_session").await?
    else {
        return Ok(());
    };
    assert!(
        outcome.saw_exec_start_success,
        "expected execStart success response"
    );
    assert!(
        outcome.subcommand_callbacks >= 2,
        "expected at least two subcommand callbacks before exit, got {}",
        outcome.subcommand_callbacks
    );
    assert_eq!(
        outcome.exit_code,
        Some(0),
        "approved_for_session should allow successful command exit"
    );
    Ok(())
}

#[tokio::test]
async fn approved_execpolicy_amendment_subcommand_decision_allows_exec() -> Result<()> {
    let Some(outcome) =
        run_compound_exec_with_second_subcommand_decision("approved_execpolicy_amendment").await?
    else {
        return Ok(());
    };
    assert!(
        outcome.saw_exec_start_success,
        "expected execStart success response"
    );
    assert!(
        outcome.subcommand_callbacks >= 2,
        "expected at least two subcommand callbacks before exit, got {}",
        outcome.subcommand_callbacks
    );
    assert_eq!(
        outcome.exit_code,
        Some(0),
        "approved_execpolicy_amendment should allow successful command exit"
    );
    Ok(())
}

#[tokio::test]
async fn approval_callback_ignores_unexpected_response_id() -> Result<()> {
    let Some(mut harness) = SidecarHarness::start().await? else {
        return Ok(());
    };

    harness.initialize().await?;
    harness
        .start_exec_with_command("/usr/bin/true && /usr/bin/true", None)
        .await?;

    let mut sent_wrong_id_once = false;
    let mut saw_exec_start_success = false;
    let mut saw_exec_exited = false;
    let mut exit_code = None;

    while !saw_exec_exited {
        let value = harness.read_next_message().await?;
        if let Some((id, _reason, _command)) = parse_approval_request(&value) {
            if !sent_wrong_id_once {
                harness
                    .respond_approval(
                        JsonValue::String("definitely-wrong-id".to_string()),
                        "approved",
                    )
                    .await?;
                sent_wrong_id_once = true;
            }
            harness.respond_approval(id, "approved").await?;
            continue;
        }

        if value.get("id").and_then(JsonValue::as_i64) == Some(EXEC_START_REQUEST_ID)
            && value.get("result").is_some()
        {
            saw_exec_start_success = true;
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

    harness.shutdown().await?;

    assert!(sent_wrong_id_once, "expected wrong-id response to be sent");
    assert!(
        saw_exec_start_success,
        "expected execStart success despite wrong callback id response"
    );
    assert_eq!(exit_code, Some(0), "expected successful command exit");
    Ok(())
}

#[tokio::test]
async fn malformed_approval_response_terminates_sidecar() -> Result<()> {
    let Some(mut harness) = SidecarHarness::start().await? else {
        return Ok(());
    };

    harness.initialize().await?;
    harness
        .start_exec_with_command("/usr/bin/true && /usr/bin/true", None)
        .await?;

    loop {
        let value = harness.read_next_message().await?;
        if let Some((id, _reason, _command)) = parse_approval_request(&value) {
            harness
                .write_json_line(&serde_json::json!({
                    "jsonrpc": JSONRPC_VERSION,
                    "id": id,
                    "result": {}
                }))
                .await?;
            break;
        }
    }

    let status = timeout(Duration::from_secs(3), harness.child.wait())
        .await
        .context("timed out waiting for sidecar crash on malformed callback response")??;
    assert!(
        !status.success(),
        "sidecar should fail fast on malformed callback response"
    );
    Ok(())
}

#[tokio::test]
async fn returns_jsonrpc_error_for_unknown_method() -> Result<()> {
    let Some(mut harness) = SidecarHarness::start().await? else {
        return Ok(());
    };

    harness.initialize().await?;
    harness
        .write_json_line(&serde_json::json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": 55,
            "method": "zsh/notRealMethod",
            "params": {}
        }))
        .await?;
    let response = harness.wait_for_response(55).await?;
    assert_eq!(
        response.pointer("/error/code"),
        Some(&JsonValue::from(-32601))
    );

    harness.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn returns_jsonrpc_invalid_params_for_exec_start_with_empty_command() -> Result<()> {
    let Some(mut harness) = SidecarHarness::start().await? else {
        return Ok(());
    };

    harness.initialize().await?;
    harness
        .write_json_line(&serde_json::json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": EXEC_START_REQUEST_ID,
            "method": "zsh/execStart",
            "params": {
                "execId": "exec-invalid",
                "command": [],
                "cwd": std::env::current_dir()?.to_string_lossy().to_string(),
                "env": {}
            }
        }))
        .await?;
    let response = harness.wait_for_response(EXEC_START_REQUEST_ID).await?;
    assert_eq!(
        response.pointer("/error/code"),
        Some(&JsonValue::from(-32602))
    );

    harness.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn exec_events_are_ordered_exec_started_before_output_and_single_exit() -> Result<()> {
    let Some(mut harness) = SidecarHarness::start().await? else {
        return Ok(());
    };

    harness.initialize().await?;
    harness
        .start_exec_with_command("/usr/bin/printf 'hi\\n'", None)
        .await?;

    let mut saw_exec_started = false;
    let mut saw_output_before_started = false;
    let mut exec_exited_count = 0usize;
    let mut saw_exec_start_success = false;

    while exec_exited_count == 0 {
        let value = harness.read_next_message().await?;
        if let Some((id, _reason, _command)) = parse_approval_request(&value) {
            harness.respond_approval(id, "approved").await?;
            continue;
        }

        match value.get("method").and_then(JsonValue::as_str) {
            Some("zsh/event/execStarted") => {
                saw_exec_started = true;
            }
            Some("zsh/event/execStdout") | Some("zsh/event/execStderr") => {
                if !saw_exec_started {
                    saw_output_before_started = true;
                }
            }
            Some("zsh/event/execExited") => {
                exec_exited_count += 1;
            }
            _ => {}
        }

        if value.get("id").and_then(JsonValue::as_i64) == Some(EXEC_START_REQUEST_ID)
            && value.get("result").is_some()
        {
            saw_exec_start_success = true;
        }
    }

    for _ in 0..4 {
        let Some(value) = harness
            .read_next_message_with_timeout(Duration::from_millis(100))
            .await?
        else {
            break;
        };
        if value.get("method").and_then(JsonValue::as_str) == Some("zsh/event/execExited") {
            exec_exited_count += 1;
        }
    }

    harness.shutdown().await?;

    assert!(
        saw_exec_start_success,
        "expected execStart success response"
    );
    assert!(saw_exec_started, "expected execStarted event");
    assert!(!saw_output_before_started, "saw output before execStarted");
    assert_eq!(exec_exited_count, 1, "expected one execExited event");
    Ok(())
}

#[tokio::test]
async fn exec_interrupt_returns_unknown_exec_id_error_in_phase1() -> Result<()> {
    let Some(mut harness) = SidecarHarness::start().await? else {
        return Ok(());
    };

    harness.initialize().await?;
    harness
        .write_json_line(&serde_json::json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": 77,
            "method": "zsh/execInterrupt",
            "params": {
                "execId": "exec-does-not-exist"
            }
        }))
        .await?;
    let response = harness.wait_for_response(77).await?;
    assert_eq!(
        response.pointer("/error/code"),
        Some(&JsonValue::from(-32002))
    );

    harness.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn exec_stdin_and_resize_return_not_supported_error_in_phase1() -> Result<()> {
    let Some(mut harness) = SidecarHarness::start().await? else {
        return Ok(());
    };

    harness.initialize().await?;
    harness
        .write_json_line(&serde_json::json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": 78,
            "method": "zsh/execStdin",
            "params": {
                "execId": "exec-does-not-exist",
                "chunkBase64": "aGk="
            }
        }))
        .await?;
    let stdin_response = harness.wait_for_response(78).await?;
    assert_eq!(
        stdin_response.pointer("/error/code"),
        Some(&JsonValue::from(-32004))
    );

    harness
        .write_json_line(&serde_json::json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": 79,
            "method": "zsh/execResize",
            "params": {
                "execId": "exec-does-not-exist",
                "cols": 80,
                "rows": 24
            }
        }))
        .await?;
    let resize_response = harness.wait_for_response(79).await?;
    assert_eq!(
        resize_response.pointer("/error/code"),
        Some(&JsonValue::from(-32004))
    );

    harness.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn wrapper_mode_with_invalid_socket_fails_fast() -> Result<()> {
    let sidecar = env!("CARGO_BIN_EXE_codex-zsh-sidecar");
    let mut child = Command::new(sidecar)
        .arg("/usr/bin/true")
        .env(WRAPPER_MODE_ENV_VAR, "1")
        .env(
            WRAPPER_SOCKET_ENV_VAR,
            "/tmp/definitely-not-a-real-codex-zsh-wrapper.sock",
        )
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("spawn wrapper mode sidecar process")?;

    let status = timeout(Duration::from_secs(3), child.wait())
        .await
        .context("timed out waiting for wrapper mode process failure")??;
    let stderr = child
        .stderr
        .take()
        .context("missing stderr for wrapper mode process")?;
    let mut lines = BufReader::new(stderr).lines();
    let mut stderr_text = String::new();
    while let Some(line) = lines.next_line().await? {
        stderr_text.push_str(&line);
        stderr_text.push('\n');
    }

    assert!(
        !status.success(),
        "wrapper mode should fail when socket path is invalid"
    );
    assert!(
        stderr_text.contains("wrapper socket"),
        "expected wrapper socket failure message, got: {stderr_text}"
    );
    Ok(())
}

#[tokio::test]
async fn wrapper_mode_sends_origin_metadata_from_env() -> Result<()> {
    let sidecar = env!("CARGO_BIN_EXE_codex-zsh-sidecar");
    let socket_path = std::env::temp_dir().join(format!(
        "czs-wrapper-test-{}.sock",
        uuid::Uuid::new_v4().as_simple()
    ));
    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("bind test listener at {}", socket_path.display()))?;

    let mut child = Command::new(sidecar)
        .arg("/usr/bin/true")
        .env(WRAPPER_MODE_ENV_VAR, "1")
        .env(
            WRAPPER_SOCKET_ENV_VAR,
            socket_path.to_string_lossy().to_string(),
        )
        .env(WRAPPER_ORIGIN_ENV_VAR, "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("spawn wrapper mode sidecar process")?;

    let (mut stream, _) = timeout(Duration::from_secs(3), listener.accept())
        .await
        .context("timed out waiting for wrapper connection")??;
    let mut request_buf = Vec::new();
    stream
        .read_to_end(&mut request_buf)
        .await
        .context("read wrapper request payload")?;
    let request: JsonValue =
        serde_json::from_slice(&request_buf).context("parse wrapper request payload")?;
    assert_eq!(
        request.pointer("/origin"),
        Some(&JsonValue::from("login_startup"))
    );
    stream
        .write_all(br#"{"action":"run","reason":null}"#)
        .await
        .context("write wrapper allow response")?;
    stream.shutdown().await.context("shutdown wrapper stream")?;

    let status = timeout(Duration::from_secs(3), child.wait())
        .await
        .context("timed out waiting for wrapper mode process exit")??;
    let _ = std::fs::remove_file(&socket_path);
    assert!(
        status.success(),
        "wrapper mode should succeed when action=run"
    );
    Ok(())
}

struct SidecarHarness {
    child: Child,
    stdin: ChildStdin,
    lines: tokio::io::Lines<BufReader<ChildStdout>>,
    zsh_path: std::path::PathBuf,
}

impl SidecarHarness {
    async fn start() -> Result<Option<Self>> {
        let Some(zsh_path) = std::env::var_os("CODEX_TEST_ZSH_PATH") else {
            eprintln!("skipping direct sidecar protocol test: CODEX_TEST_ZSH_PATH is not set");
            return Ok(None);
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

        let stdin = child.stdin.take().context("missing sidecar stdin")?;
        let stdout = child.stdout.take().context("missing sidecar stdout")?;

        Ok(Some(Self {
            child,
            stdin,
            lines: BufReader::new(stdout).lines(),
            zsh_path,
        }))
    }

    async fn initialize(&mut self) -> Result<()> {
        self.write_json_line(&serde_json::json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": 1,
            "method": "zsh/initialize",
            "params": {
                "sessionId": "test-session"
            }
        }))
        .await?;
        self.wait_for_response(1).await?;
        Ok(())
    }

    async fn start_exec_with_command(
        &mut self,
        shell_command: &str,
        approval_reason: Option<&str>,
    ) -> Result<()> {
        self.write_json_line(&serde_json::json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": EXEC_START_REQUEST_ID,
            "method": "zsh/execStart",
            "params": {
                "execId": "exec-test-1",
                "command": [self.zsh_path.to_string_lossy(), "-fc", shell_command],
                "cwd": std::env::current_dir()?.to_string_lossy().to_string(),
                "env": {},
                "approvalReason": approval_reason
            }
        }))
        .await
    }

    async fn respond_approval(&mut self, id: JsonValue, decision: &str) -> Result<()> {
        self.write_json_line(&serde_json::json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": id,
            "result": {
                "decision": decision
            }
        }))
        .await
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.write_json_line(&serde_json::json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": 3,
            "method": "zsh/shutdown",
            "params": {
                "graceMs": 100
            }
        }))
        .await?;
        self.wait_for_response(3).await?;

        let status = timeout(Duration::from_secs(3), self.child.wait())
            .await
            .context("timed out waiting for sidecar process exit")??;
        assert!(status.success(), "sidecar should exit cleanly");
        Ok(())
    }

    async fn read_next_message(&mut self) -> Result<JsonValue> {
        let line = timeout(Duration::from_secs(10), self.lines.next_line())
            .await
            .context("timed out reading sidecar output")??
            .context("sidecar stdout closed unexpectedly")?;
        serde_json::from_str(&line).with_context(|| format!("parse sidecar JSON line: {line}"))
    }

    async fn read_next_message_with_timeout(
        &mut self,
        duration: Duration,
    ) -> Result<Option<JsonValue>> {
        let line = match timeout(duration, self.lines.next_line()).await {
            Ok(line) => line?,
            Err(_) => return Ok(None),
        };
        let Some(line) = line else {
            return Ok(None);
        };
        let value = serde_json::from_str(&line)
            .with_context(|| format!("parse sidecar JSON line: {line}"))?;
        Ok(Some(value))
    }

    async fn wait_for_response(&mut self, id: i64) -> Result<JsonValue> {
        loop {
            let value = self.read_next_message().await?;
            if value.get("id").and_then(JsonValue::as_i64) == Some(id) {
                return Ok(value);
            }
        }
    }

    async fn write_json_line(&mut self, value: &JsonValue) -> Result<()> {
        let encoded = serde_json::to_string(value).context("serialize JSON line")?;
        self.stdin
            .write_all(encoded.as_bytes())
            .await
            .context("write JSON line")?;
        self.stdin
            .write_all(b"\n")
            .await
            .context("write line break")?;
        self.stdin.flush().await.context("flush stdin")
    }
}

fn parse_approval_request(value: &JsonValue) -> Option<(JsonValue, Option<String>, Vec<String>)> {
    if value.get("method").and_then(JsonValue::as_str) != Some("zsh/requestApproval") {
        return None;
    }
    let id = value.get("id")?.clone();
    let reason = value
        .pointer("/params/reason")
        .and_then(JsonValue::as_str)
        .map(ToString::to_string);
    let command = value
        .pointer("/params/command")
        .and_then(JsonValue::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(JsonValue::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    Some((id, reason, command))
}

struct CompoundExecOutcome {
    saw_exec_start_success: bool,
    subcommand_callbacks: usize,
    exit_code: Option<i32>,
}

async fn run_compound_exec_with_second_subcommand_decision(
    second_subcommand_decision: &str,
) -> Result<Option<CompoundExecOutcome>> {
    let Some(mut harness) = SidecarHarness::start().await? else {
        return Ok(None);
    };

    harness.initialize().await?;
    harness
        .start_exec_with_command("/usr/bin/true && /usr/bin/true", None)
        .await?;

    let mut subcommand_callbacks = 0usize;
    let mut saw_exec_start_success = false;
    let mut saw_exec_exited = false;
    let mut exit_code = None;

    while !saw_exec_exited {
        let value = harness.read_next_message().await?;

        if let Some((id, _reason, command)) = parse_approval_request(&value) {
            if command.first().is_some_and(|c| c == "/usr/bin/true") {
                subcommand_callbacks += 1;
                if subcommand_callbacks == 2 {
                    harness
                        .respond_approval(id, second_subcommand_decision)
                        .await?;
                } else {
                    harness.respond_approval(id, "approved").await?;
                }
            } else {
                harness.respond_approval(id, "approved").await?;
            }
            continue;
        }

        if value.get("id").and_then(JsonValue::as_i64) == Some(EXEC_START_REQUEST_ID)
            && value.get("result").is_some()
        {
            saw_exec_start_success = true;
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

    harness.shutdown().await?;
    Ok(Some(CompoundExecOutcome {
        saw_exec_start_success,
        subcommand_callbacks,
        exit_code,
    }))
}
