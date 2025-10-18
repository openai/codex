#![cfg(not(target_os = "windows"))]

use anyhow::Result;
use codex_core::UnifiedExecMode;
use codex_core::UnifiedExecRequest;
use codex_core::UnifiedExecSessionManager;
#[cfg(unix)]
use core_test_support::skip_if_sandbox;
use serde_json::Value;
use tokio::time::Duration;

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_manager_supports_interactive_cat() -> Result<()> {
    skip_if_sandbox!(Ok(()));

    let manager = UnifiedExecSessionManager::default();
    let result = manager
        .handle_request(UnifiedExecRequest {
            mode: UnifiedExecMode::Start {
                cmd: std::borrow::Cow::Borrowed("cat"),
                yield_time_ms: Some(200),
                max_output_tokens: Some(1_000),
                shell: Some("/bin/sh"),
                login: Some(false),
                cwd: None,
            },
            output_chunk_id: Some(true),
            output_wall_time: Some(true),
            output_json: Some(false),
        })
        .await?;

    let session_id = result.metadata.session_id.expect("expected session id");
    let poll = manager
        .handle_request(UnifiedExecRequest {
            mode: UnifiedExecMode::Write {
                session_id,
                chars: "hello unified exec\n",
                yield_time_ms: Some(500),
                max_output_tokens: Some(1_000),
            },
            output_chunk_id: Some(false),
            output_wall_time: Some(false),
            output_json: Some(true),
        })
        .await?;

    let output = poll.content.into_string();
    assert!(output.contains("hello unified exec"));

    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_manager_streams_large_output() -> Result<()> {
    skip_if_sandbox!(Ok(()));

    let manager = UnifiedExecSessionManager::default();
    let script = r#"python3 - <<'PY'
import sys
for _ in range(3):
    sys.stdout.write("TAIL-MARKER\n")
    sys.stdout.flush()
PY
"#;

    let start = manager
        .handle_request(UnifiedExecRequest {
            mode: UnifiedExecMode::Start {
                cmd: std::borrow::Cow::Borrowed(script),
                yield_time_ms: Some(500),
                max_output_tokens: Some(5_000),
                shell: Some("/bin/sh"),
                login: Some(false),
                cwd: None,
            },
            output_chunk_id: None,
            output_wall_time: None,
            output_json: Some(false),
        })
        .await?;

    let output = start.content.into_string();
    assert!(output.contains("TAIL-MARKER"));

    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_manager_handles_timeout_then_poll() -> Result<()> {
    skip_if_sandbox!(Ok(()));

    let manager = UnifiedExecSessionManager::default();
    let result = manager
        .handle_request(UnifiedExecRequest {
            mode: UnifiedExecMode::Start {
                cmd: std::borrow::Cow::Borrowed("sleep 0.1; echo ready"),
                yield_time_ms: Some(10),
                max_output_tokens: Some(1_000),
                shell: Some("/bin/sh"),
                login: Some(false),
                cwd: None,
            },
            output_chunk_id: None,
            output_wall_time: None,
            output_json: Some(false),
        })
        .await?;

    if let Some(session_id) = result.metadata.session_id {
        tokio::time::sleep(Duration::from_millis(200)).await;
        match manager
            .handle_request(UnifiedExecRequest {
                mode: UnifiedExecMode::Write {
                    session_id,
                    chars: "",
                    yield_time_ms: Some(500),
                    max_output_tokens: Some(1_000),
                },
                output_chunk_id: None,
                output_wall_time: None,
                output_json: Some(false),
            })
            .await
        {
            Ok(poll) => assert!(poll.content.into_string().contains("ready")),
            Err(codex_core::UnifiedExecError::SessionExited { .. }) => {}
            Err(other) => return Err(other.into()),
        }
    } else {
        assert!(result.content.into_string().contains("ready"));
    }

    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_json_output_matches_metadata() -> Result<()> {
    skip_if_sandbox!(Ok(()));

    let manager = UnifiedExecSessionManager::default();
    let command = "printf 'ready\\n' && read dummy";

    let result = manager
        .handle_request(UnifiedExecRequest {
            mode: UnifiedExecMode::Start {
                cmd: std::borrow::Cow::Borrowed(command),
                yield_time_ms: Some(500),
                max_output_tokens: Some(1_000),
                shell: Some("/bin/bash"),
                login: Some(false),
                cwd: None,
            },
            output_chunk_id: Some(true),
            output_wall_time: Some(true),
            output_json: Some(true),
        })
        .await?;

    let codex_core::UnifiedExecResult { content, metadata } = result;

    let body = content.into_string();
    let json: Value = serde_json::from_str(&body)?;

    assert!(json.get("chunk_id").is_some());
    assert!(json.get("wall_time").is_some());

    let session_id = metadata.session_id.expect("expected running session");
    assert_eq!(json["session_id"].as_i64(), Some(i64::from(session_id)));

    let output = json["output"]
        .as_object()
        .expect("output is object with numbered lines");
    assert_eq!(
        output.get("1").and_then(Value::as_str),
        Some("ready"),
        "expected first output line to contain ready"
    );

    assert_eq!(metadata.exec_cmd.as_deref(), Some(command));

    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_respects_output_preferences() -> Result<()> {
    skip_if_sandbox!(Ok(()));

    let manager = UnifiedExecSessionManager::default();

    let result = manager
        .handle_request(UnifiedExecRequest {
            mode: UnifiedExecMode::Start {
                cmd: std::borrow::Cow::Borrowed("printf 'ready\\n' && read dummy"),
                yield_time_ms: Some(500),
                max_output_tokens: Some(1_000),
                shell: Some("/bin/bash"),
                login: Some(false),
                cwd: None,
            },
            output_chunk_id: Some(false),
            output_wall_time: Some(false),
            output_json: Some(false),
        })
        .await?;

    let codex_core::UnifiedExecResult { content, metadata } = result;

    assert_eq!(
        metadata.exec_cmd.as_deref(),
        Some("printf 'ready\\n' && read dummy")
    );
    assert!(
        metadata.session_id.is_some(),
        "session should remain active when waiting for stdin input"
    );

    let text = content.into_string();
    assert!(
        !text.contains("Chunk ID:"),
        "chunk metadata should be omitted when output_chunk_id is false: {text}"
    );
    assert!(
        !text.contains("Wall time:"),
        "wall time metadata should be omitted when output_wall_time is false: {text}"
    );
    assert!(
        text.contains("Process running with session ID"),
        "expected running-session metadata in textual response: {text}"
    );
    assert!(
        text.contains("ready"),
        "expected command output to appear in textual response: {text}"
    );

    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_reports_truncation_metadata() -> Result<()> {
    skip_if_sandbox!(Ok(()));

    let manager = UnifiedExecSessionManager::default();
    let script = r#"python3 - <<'PY'
import sys
sys.stdout.write("X" * 2048)
sys.stdout.flush()
PY
"#;

    let result = manager
        .handle_request(UnifiedExecRequest {
            mode: UnifiedExecMode::Start {
                cmd: std::borrow::Cow::Borrowed(script),
                yield_time_ms: Some(500),
                max_output_tokens: Some(1),
                shell: Some("/bin/sh"),
                login: Some(false),
                cwd: None,
            },
            output_chunk_id: Some(true),
            output_wall_time: Some(true),
            output_json: Some(false),
        })
        .await?;

    let codex_core::UnifiedExecResult { content, metadata } = result;

    assert!(
        metadata.original_token_count.is_some_and(|count| count > 0),
        "expected original_token_count metadata when truncation occurs"
    );

    let text = content.into_string();
    assert!(
        text.contains("tokens truncated"),
        "expected truncation notice in textual output: {text}"
    );

    Ok(())
}
