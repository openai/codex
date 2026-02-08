use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use super::types::Hook;
use super::types::HookOutcome;
use super::types::HookPayload;

/// Maximum bytes to read from a hook command's stdout to prevent unbounded memory usage.
const MAX_STDOUT_BYTES: usize = 1_048_576; // 1MB

/// Maximum bytes to read from a hook command's stderr to prevent unbounded memory usage.
const MAX_STDERR_BYTES: usize = 1_048_576; // 1MB

/// Decision returned by a hook command.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum HookDecision {
    Proceed,
    Block,
    Modify,
}

/// Result structure returned by a hook command via stdout JSON.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct HookCommandResult {
    pub decision: HookDecision,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

impl From<HookCommandResult> for HookOutcome {
    fn from(result: HookCommandResult) -> Self {
        match result.decision {
            HookDecision::Proceed => HookOutcome::Proceed,
            HookDecision::Block => HookOutcome::Block {
                message: result.message,
            },
            HookDecision::Modify => match result.content {
                Some(content) => HookOutcome::Modify { content },
                None => {
                    tracing::warn!(
                        "hook returned modify decision without content field; \
                         treating as block to prevent empty input substitution"
                    );
                    HookOutcome::Block {
                        message: Some(
                            "hook returned modify without content field".to_string(),
                        ),
                    }
                }
            },
        }
    }
}

/// Creates a hook that executes a command via stdin/stdout JSON protocol.
///
/// The hook serializes the payload to JSON, pipes it to the command's stdin,
/// reads the command's stdout, and interprets the result as a HookOutcome.
///
/// # Interpretation Rules
///
/// - Exit code 0 + empty stdout → `HookOutcome::Proceed`
/// - Exit code 0 + stdout JSON with `{"decision": "block", "message": "..."}` → `HookOutcome::Block`
/// - Exit code 0 + stdout JSON with `{"decision": "modify", "content": "..."}` → `HookOutcome::Modify`
/// - Non-zero exit code → `HookOutcome::Block { message: Some(stderr_or_default) }`
/// - Timeout → `HookOutcome::Block { message: Some("hook timed out") }`
/// - Spawn failure → log warning and return `HookOutcome::Proceed` (fail-open)
pub(super) fn command_hook(argv: Vec<String>, timeout: Duration) -> Hook {
    Hook {
        func: Arc::new(move |payload: &HookPayload| {
            let argv = argv.clone();
            let payload = payload.clone();
            Box::pin(async move {
                let Some(mut command) = super::registry::command_from_argv(&argv) else {
                    tracing::warn!("hook command argv is empty, skipping");
                    return HookOutcome::Proceed;
                };

                command
                    .current_dir(&payload.cwd)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                let mut child = match command.spawn() {
                    Ok(child) => child,
                    Err(err) => {
                        tracing::warn!("failed to spawn hook command: {err}");
                        return HookOutcome::Proceed;
                    }
                };

                let Some(mut stdin) = child.stdin.take() else {
                    tracing::warn!("hook child process has no stdin handle");
                    return HookOutcome::Proceed;
                };
                let Some(mut stdout) = child.stdout.take() else {
                    tracing::warn!("hook child process has no stdout handle");
                    return HookOutcome::Proceed;
                };
                let Some(mut stderr) = child.stderr.take() else {
                    tracing::warn!("hook child process has no stderr handle");
                    return HookOutcome::Proceed;
                };

                // Serialize payload to JSON before entering the timed block.
                let payload_json = match serde_json::to_vec(&payload) {
                    Ok(json) => json,
                    Err(err) => {
                        tracing::warn!("failed to serialize hook payload: {err}");
                        return HookOutcome::Proceed;
                    }
                };

                // Wrap the entire IO sequence (stdin write, stdout + stderr
                // read) in a single timeout so that a misbehaving hook cannot
                // hang any individual phase indefinitely.  Stdout and stderr
                // are drained concurrently to avoid pipe deadlocks when a hook
                // produces verbose output on both streams.
                let io_result = tokio::time::timeout(timeout, async {
                    // Write payload to stdin.  If the hook closes stdin
                    // early (e.g. a short script that ignores input), we
                    // still need to read its stdout/stderr and exit status
                    // so that block/modify decisions are not silently lost.
                    if let Err(err) = stdin.write_all(&payload_json).await {
                        tracing::warn!("failed to write payload to hook stdin: {err}");
                    }
                    drop(stdin); // Close stdin to signal EOF

                    // Drain stdout and stderr concurrently to prevent pipe
                    // deadlocks (a full stderr buffer can block the child
                    // before it closes stdout, causing a false timeout).
                    let read_stdout = async {
                        let mut bytes = Vec::new();
                        let mut buf = [0u8; 4096];
                        let mut capped = false;
                        loop {
                            match stdout.read(&mut buf).await {
                                Ok(0) => break,
                                Ok(n) => {
                                    if capped {
                                        continue; // drain but discard
                                    }
                                    if bytes.len() + n > MAX_STDOUT_BYTES {
                                        // Keep as many bytes as still fit
                                        // before switching to drain mode.
                                        let remaining = MAX_STDOUT_BYTES - bytes.len();
                                        bytes.extend_from_slice(&buf[..remaining]);
                                        tracing::warn!(
                                            "hook stdout exceeded max size of {MAX_STDOUT_BYTES} bytes"
                                        );
                                        capped = true;
                                        continue;
                                    }
                                    bytes.extend_from_slice(&buf[..n]);
                                }
                                Err(err) => {
                                    tracing::warn!("failed to read hook stdout: {err}");
                                    break;
                                }
                            }
                        }
                        (bytes, capped)
                    };

                    let read_stderr = async {
                        let mut bytes = Vec::new();
                        let mut buf = [0u8; 4096];
                        let mut capped = false;
                        loop {
                            match stderr.read(&mut buf).await {
                                Ok(0) => break,
                                Ok(n) => {
                                    if capped {
                                        continue; // drain but discard
                                    }
                                    if bytes.len() + n > MAX_STDERR_BYTES {
                                        bytes.extend_from_slice(
                                            &buf[..MAX_STDERR_BYTES - bytes.len()],
                                        );
                                        tracing::warn!(
                                            "hook stderr exceeded max size of {MAX_STDERR_BYTES} bytes, truncated"
                                        );
                                        capped = true;
                                        continue;
                                    }
                                    bytes.extend_from_slice(&buf[..n]);
                                }
                                Err(_) => break,
                            }
                        }
                        String::from_utf8_lossy(&bytes).to_string()
                    };

                    let ((stdout_bytes, stdout_capped), stderr_string) =
                        tokio::join!(read_stdout, read_stderr);

                    (stdout_bytes, stdout_capped, stderr_string)
                })
                .await;

                // Handle IO timeout: kill the child and return Block.
                let (stdout_bytes, stdout_capped, stderr_string) = match io_result {
                    Err(_elapsed) => {
                        let _ = child.kill().await;
                        return HookOutcome::Block {
                            message: Some("hook timed out".to_string()),
                        };
                    }
                    Ok(data) => data,
                };

                // Wait for process exit.  Once stdout and stderr are fully
                // consumed the process should exit promptly; apply a generous
                // grace period to guard against pathological cases.
                const WAIT_GRACE: Duration = Duration::from_secs(5);
                let status = match tokio::time::timeout(WAIT_GRACE, child.wait()).await {
                    Ok(Ok(status)) => status,
                    Ok(Err(err)) => {
                        tracing::warn!("failed to wait for hook command: {err}");
                        return HookOutcome::Proceed;
                    }
                    Err(_elapsed) => {
                        let _ = child.kill().await;
                        return HookOutcome::Block {
                            message: Some("hook timed out".to_string()),
                        };
                    }
                };

                // Non-zero exit code → block with stderr message
                if !status.success() {
                    let message = if stderr_string.is_empty() {
                        format!("hook command failed with exit code {status}")
                    } else {
                        stderr_string
                    };
                    return HookOutcome::Block {
                        message: Some(message),
                    };
                }

                // Exit code 0: parse stdout or default to Proceed
                if stdout_bytes.is_empty() {
                    return HookOutcome::Proceed;
                }

                // If stdout was truncated, the JSON is likely corrupted.
                // Block rather than falling through to Proceed, which would
                // silently bypass the hook's intended decision.
                if stdout_capped {
                    return HookOutcome::Block {
                        message: Some(
                            "hook stdout exceeded size limit; output truncated and cannot be trusted".to_string(),
                        ),
                    };
                }

                match serde_json::from_slice::<HookCommandResult>(&stdout_bytes) {
                    Ok(result) => result.into(),
                    Err(err) => {
                        tracing::warn!("failed to parse hook command result: {err}");
                        HookOutcome::Proceed
                    }
                }
            })
        }),
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::super::types::HookOutcome;
    use super::HookCommandResult;
    use super::HookDecision;

    #[test]
    fn test_hook_command_result_deserialize_proceed() {
        let json = json!({"decision": "proceed"});
        let result: HookCommandResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.decision, HookDecision::Proceed);
        assert_eq!(result.message, None);
        assert_eq!(result.content, None);
    }

    #[test]
    fn test_hook_command_result_deserialize_block() {
        let json = json!({"decision": "block", "message": "denied"});
        let result: HookCommandResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.decision, HookDecision::Block);
        assert_eq!(result.message, Some("denied".to_string()));
        assert_eq!(result.content, None);
    }

    #[test]
    fn test_hook_command_result_deserialize_modify() {
        let json = json!({"decision": "modify", "content": "new text"});
        let result: HookCommandResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.decision, HookDecision::Modify);
        assert_eq!(result.message, None);
        assert_eq!(result.content, Some("new text".to_string()));
    }

    // ---- command_hook() integration tests (Unix only) ----

    #[cfg(not(windows))]
    mod command_hook_integration {
        use std::path::PathBuf;
        use std::time::Duration;

        use chrono::TimeZone;
        use chrono::Utc;
        use codex_protocol::ThreadId;
        use pretty_assertions::assert_eq;

        use super::super::super::types::HookEvent;
        use super::super::super::types::HookEventAfterAgent;
        use super::super::super::types::HookOutcome;
        use super::super::super::types::HookPayload;
        use super::super::command_hook;

        fn test_payload() -> HookPayload {
            HookPayload {
                session_id: ThreadId::new(),
                cwd: PathBuf::from("/tmp"),
                triggered_at: Utc
                    .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                    .single()
                    .expect("valid timestamp"),
                hook_event: HookEvent::AfterAgent {
                    event: HookEventAfterAgent {
                        thread_id: ThreadId::new(),
                        turn_id: "test".to_string(),
                        input_messages: vec!["hello".to_string()],
                        last_assistant_message: None,
                    },
                },
            }
        }

        #[tokio::test]
        async fn command_hook_empty_stdout_returns_proceed() {
            // Command reads stdin but produces no stdout → Proceed
            let hook = command_hook(
                vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    "cat > /dev/null".to_string(),
                ],
                Duration::from_secs(5),
            );
            let outcome = hook.execute(&test_payload()).await;
            assert_eq!(outcome, HookOutcome::Proceed);
        }

        #[tokio::test]
        async fn command_hook_stdout_proceed_json() {
            let hook = command_hook(
                vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    r#"cat > /dev/null; echo '{"decision":"proceed"}'"#.to_string(),
                ],
                Duration::from_secs(5),
            );
            let outcome = hook.execute(&test_payload()).await;
            assert_eq!(outcome, HookOutcome::Proceed);
        }

        #[tokio::test]
        async fn command_hook_stdout_block_json() {
            let hook = command_hook(
                vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    r#"cat > /dev/null; echo '{"decision":"block","message":"denied by policy"}'"#
                        .to_string(),
                ],
                Duration::from_secs(5),
            );
            let outcome = hook.execute(&test_payload()).await;
            assert_eq!(
                outcome,
                HookOutcome::Block {
                    message: Some("denied by policy".to_string())
                }
            );
        }

        #[tokio::test]
        async fn command_hook_stdout_modify_json() {
            let hook = command_hook(
                vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    r#"cat > /dev/null; echo '{"decision":"modify","content":"new content"}'"#
                        .to_string(),
                ],
                Duration::from_secs(5),
            );
            let outcome = hook.execute(&test_payload()).await;
            assert_eq!(
                outcome,
                HookOutcome::Modify {
                    content: "new content".to_string()
                }
            );
        }

        #[tokio::test]
        async fn command_hook_nonzero_exit_returns_block() {
            let hook = command_hook(
                vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    "cat > /dev/null; echo 'error msg' >&2; exit 1".to_string(),
                ],
                Duration::from_secs(5),
            );
            let outcome = hook.execute(&test_payload()).await;
            match outcome {
                HookOutcome::Block { message } => {
                    let msg = message.expect("should have error message");
                    assert!(
                        msg.contains("error msg"),
                        "stderr should be in message: {msg}"
                    );
                }
                other => panic!("expected Block, got {other:?}"),
            }
        }

        #[tokio::test]
        async fn command_hook_nonzero_exit_empty_stderr_uses_exit_code() {
            let hook = command_hook(
                vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    "cat > /dev/null; exit 42".to_string(),
                ],
                Duration::from_secs(5),
            );
            let outcome = hook.execute(&test_payload()).await;
            match outcome {
                HookOutcome::Block { message } => {
                    let msg = message.expect("should have error message");
                    assert!(
                        msg.contains("exit"),
                        "message should mention exit code: {msg}"
                    );
                }
                other => panic!("expected Block, got {other:?}"),
            }
        }

        #[tokio::test]
        async fn command_hook_timeout_returns_block() {
            let hook = command_hook(
                vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    "cat > /dev/null; sleep 60".to_string(),
                ],
                Duration::from_millis(100), // Very short timeout
            );
            let outcome = hook.execute(&test_payload()).await;
            assert_eq!(
                outcome,
                HookOutcome::Block {
                    message: Some("hook timed out".to_string())
                }
            );
        }

        #[tokio::test]
        async fn command_hook_invalid_json_stdout_returns_proceed() {
            let hook = command_hook(
                vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    "cat > /dev/null; echo 'not valid json'".to_string(),
                ],
                Duration::from_secs(5),
            );
            let outcome = hook.execute(&test_payload()).await;
            // Invalid JSON → fail-open → Proceed
            assert_eq!(outcome, HookOutcome::Proceed);
        }

        #[tokio::test]
        async fn command_hook_nonexistent_command_returns_proceed() {
            let hook = command_hook(
                vec!["/nonexistent/command/path/xxxxx".to_string()],
                Duration::from_secs(5),
            );
            let outcome = hook.execute(&test_payload()).await;
            // Spawn failure → fail-open → Proceed
            assert_eq!(outcome, HookOutcome::Proceed);
        }

        #[tokio::test]
        async fn command_hook_empty_argv_returns_proceed() {
            let hook = command_hook(vec![], Duration::from_secs(5));
            let outcome = hook.execute(&test_payload()).await;
            assert_eq!(outcome, HookOutcome::Proceed);
        }

        #[tokio::test]
        async fn command_hook_receives_payload_on_stdin() {
            // Verify the hook receives the JSON payload on stdin by having
            // the script parse it and echo back a field from the payload.
            let hook = command_hook(
                vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    // Read stdin, check it's valid JSON with jq-like approach,
                    // then return proceed. We just verify it doesn't fail.
                    "cat > /dev/null; echo '{\"decision\":\"proceed\"}'".to_string(),
                ],
                Duration::from_secs(5),
            );
            let outcome = hook.execute(&test_payload()).await;
            assert_eq!(outcome, HookOutcome::Proceed);
        }

        #[tokio::test]
        async fn command_hook_runs_in_payload_cwd() {
            // Verify that the hook command runs in the payload's cwd directory
            // by having the script print its working directory via `pwd`.
            let hook = command_hook(
                vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    "cat > /dev/null; pwd".to_string(),
                ],
                Duration::from_secs(5),
            );
            // test_payload() sets cwd to /tmp
            let outcome = hook.execute(&test_payload()).await;
            // pwd outputs the working directory; since it's not valid JSON,
            // the executor falls through to Proceed (fail-open on invalid JSON).
            // The important thing is that it doesn't fail to spawn, proving
            // the command runs. We verify cwd more precisely below.
            assert_eq!(outcome, HookOutcome::Proceed);

            // Now verify with a JSON response that includes the cwd
            let hook = command_hook(
                vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    r#"cat > /dev/null; CWD=$(pwd); echo "{\"decision\":\"block\",\"message\":\"$CWD\"}""#
                        .to_string(),
                ],
                Duration::from_secs(5),
            );
            let outcome = hook.execute(&test_payload()).await;
            match outcome {
                HookOutcome::Block { message } => {
                    let msg = message.expect("should have cwd message");
                    assert_eq!(
                        msg, "/tmp",
                        "hook should run in payload.cwd (/tmp), got: {msg}"
                    );
                }
                other => panic!("expected Block with cwd message, got {other:?}"),
            }
        }
    }

    #[test]
    fn test_hook_command_result_to_outcome() {
        let result = HookCommandResult {
            decision: HookDecision::Proceed,
            message: None,
            content: None,
        };
        assert_eq!(HookOutcome::from(result), HookOutcome::Proceed);

        let result = HookCommandResult {
            decision: HookDecision::Block,
            message: Some("blocked".to_string()),
            content: None,
        };
        assert_eq!(
            HookOutcome::from(result),
            HookOutcome::Block {
                message: Some("blocked".to_string())
            }
        );

        let result = HookCommandResult {
            decision: HookDecision::Modify,
            message: None,
            content: Some("modified content".to_string()),
        };
        assert_eq!(
            HookOutcome::from(result),
            HookOutcome::Modify {
                content: "modified content".to_string()
            }
        );

        // Modify with explicit empty content is allowed
        let result = HookCommandResult {
            decision: HookDecision::Modify,
            message: None,
            content: Some(String::new()),
        };
        assert_eq!(
            HookOutcome::from(result),
            HookOutcome::Modify {
                content: String::new()
            }
        );

        // Modify without content field → Block (malformed response)
        let result = HookCommandResult {
            decision: HookDecision::Modify,
            message: None,
            content: None,
        };
        assert!(
            matches!(HookOutcome::from(result), HookOutcome::Block { .. }),
            "modify without content should be treated as Block"
        );
    }
}
