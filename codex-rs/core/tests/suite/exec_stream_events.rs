#![cfg(unix)]

use std::collections::HashMap;
use std::path::PathBuf;

use async_channel::Receiver;
use codex_core::exec::ExecParams;
use codex_core::exec::SandboxType;
use codex_core::exec::StdoutStream;
use codex_core::exec::process_exec_tool_call;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecCommandOutputDeltaEvent;
use codex_core::protocol::ExecOutputStream;
use codex_core::protocol::SandboxPolicy;

fn collect_stdout_events(rx: Receiver<Event>) -> Vec<u8> {
    let mut out = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        if let EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
            stream: ExecOutputStream::Stdout,
            chunk,
            ..
        }) = ev.msg
        {
            out.extend_from_slice(&chunk);
        }
    }
    out
}

#[tokio::test]
async fn test_exec_stdout_stream_events_echo() {
    let (tx, rx) = async_channel::unbounded::<Event>();

    let stdout_stream = StdoutStream {
        sub_id: "test-sub".to_string(),
        call_id: "call-1".to_string(),
        tx_event: tx,
    };

    let cmd = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        // Use printf for predictable behavior across shells
        "printf 'hello-world\n'".to_string(),
    ];

    let params = ExecParams {
        command: cmd,
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        timeout_ms: Some(5_000),
        env: HashMap::new(),
        with_escalated_permissions: None,
        justification: None,
    };

    let policy = SandboxPolicy::new_read_only_policy();

    let result = process_exec_tool_call(
        params,
        SandboxType::None,
        &policy,
        &None,
        Some(stdout_stream),
    )
    .await;

    let result = match result {
        Ok(r) => r,
        Err(e) => panic!("process_exec_tool_call failed: {e}"),
    };

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.text, "hello-world\n");

    let streamed = collect_stdout_events(rx);
    // We should have received at least the same contents (possibly in one chunk)
    assert_eq!(String::from_utf8_lossy(&streamed), "hello-world\n");
}

#[tokio::test]
async fn test_exec_stderr_stream_events_echo() {
    let (tx, rx) = async_channel::unbounded::<Event>();

    let stdout_stream = StdoutStream {
        sub_id: "test-sub".to_string(),
        call_id: "call-2".to_string(),
        tx_event: tx,
    };

    let cmd = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        // Write to stderr explicitly
        "printf 'oops\n' 1>&2".to_string(),
    ];

    let params = ExecParams {
        command: cmd,
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        timeout_ms: Some(5_000),
        env: HashMap::new(),
        with_escalated_permissions: None,
        justification: None,
    };

    let policy = SandboxPolicy::new_read_only_policy();

    let result = process_exec_tool_call(
        params,
        SandboxType::None,
        &policy,
        &None,
        Some(stdout_stream),
    )
    .await;

    let result = match result {
        Ok(r) => r,
        Err(e) => panic!("process_exec_tool_call failed: {e}"),
    };

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.text, "");
    assert_eq!(result.stderr.text, "oops\n");

    // Collect only stderr delta events
    let mut err = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        if let EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
            stream: ExecOutputStream::Stderr,
            chunk,
            ..
        }) = ev.msg
        {
            err.extend_from_slice(&chunk);
        }
    }
    assert_eq!(String::from_utf8_lossy(&err), "oops\n");
}

#[tokio::test]
async fn test_aggregated_output_interleaves_in_order() {
    // Spawn a shell that alternates stdout and stderr with sleeps to enforce order.
    let cmd = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "printf 'O1\\n'; sleep 0.01; printf 'E1\\n' 1>&2; sleep 0.01; printf 'O2\\n'; sleep 0.01; printf 'E2\\n' 1>&2".to_string(),
    ];

    let params = ExecParams {
        command: cmd,
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        timeout_ms: Some(5_000),
        env: HashMap::new(),
        with_escalated_permissions: None,
        justification: None,
    };

    let policy = SandboxPolicy::new_read_only_policy();

    let result = process_exec_tool_call(params, SandboxType::None, &policy, &None, None)
        .await
        .expect("process_exec_tool_call");

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.text, "O1\nO2\n");
    assert_eq!(result.stderr.text, "E1\nE2\n");
    assert_eq!(result.aggregated_output.text, "O1\nE1\nO2\nE2\n");
    assert_eq!(result.aggregated_output.truncated_after_lines, None);
}
