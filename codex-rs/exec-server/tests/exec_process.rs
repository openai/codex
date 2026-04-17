#![cfg(unix)]

mod common;

use std::sync::Arc;

use anyhow::Result;
use codex_exec_server::Environment;
use codex_exec_server::ExecBackend;
use codex_exec_server::ExecOutputStream;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcess;
use codex_exec_server::ExecProcessEvent;
use codex_exec_server::ProcessId;
use codex_exec_server::ReadResponse;
use codex_exec_server::StartedExecProcess;
use codex_exec_server::WriteStatus;
use pretty_assertions::assert_eq;
use test_case::test_case;
use tokio::sync::watch;
use tokio::time::Duration;
use tokio::time::timeout;

use common::exec_server::ExecServerHarness;
use common::exec_server::exec_server;

struct ProcessContext {
    backend: Arc<dyn ExecBackend>,
    server: Option<ExecServerHarness>,
}

#[derive(Debug, PartialEq, Eq)]
enum ProcessEventSnapshot {
    Output {
        seq: u64,
        stream: ExecOutputStream,
        text: String,
    },
    Exited {
        seq: u64,
        exit_code: i32,
    },
    Closed {
        seq: u64,
    },
}

async fn create_process_context(use_remote: bool) -> Result<ProcessContext> {
    if use_remote {
        let server = exec_server().await?;
        let environment = Environment::create(Some(server.websocket_url().to_string())).await?;
        Ok(ProcessContext {
            backend: environment.get_exec_backend(),
            server: Some(server),
        })
    } else {
        let environment = Environment::create(/*exec_server_url*/ None).await?;
        Ok(ProcessContext {
            backend: environment.get_exec_backend(),
            server: None,
        })
    }
}

async fn assert_exec_process_starts_and_exits(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let session = context
        .backend
        .start(ExecParams {
            process_id: ProcessId::from("proc-1"),
            argv: vec!["true".to_string()],
            cwd: std::env::current_dir()?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), "proc-1");
    let wake_rx = session.process.subscribe_wake();
    let (_, exit_code, closed) =
        collect_process_output_from_reads(session.process, wake_rx).await?;

    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

async fn read_process_until_change(
    session: Arc<dyn ExecProcess>,
    wake_rx: &mut watch::Receiver<u64>,
    after_seq: Option<u64>,
) -> Result<ReadResponse> {
    let response = session
        .read(after_seq, /*max_bytes*/ None, /*wait_ms*/ Some(0))
        .await?;
    if !response.chunks.is_empty() || response.closed || response.failure.is_some() {
        return Ok(response);
    }

    timeout(Duration::from_secs(2), wake_rx.changed()).await??;
    session
        .read(after_seq, /*max_bytes*/ None, /*wait_ms*/ Some(0))
        .await
        .map_err(Into::into)
}

async fn collect_process_output_from_reads(
    session: Arc<dyn ExecProcess>,
    mut wake_rx: watch::Receiver<u64>,
) -> Result<(String, Option<i32>, bool)> {
    let mut output = String::new();
    let mut exit_code = None;
    let mut after_seq = None;
    loop {
        let response =
            read_process_until_change(Arc::clone(&session), &mut wake_rx, after_seq).await?;
        if let Some(message) = response.failure {
            anyhow::bail!("process failed before closed state: {message}");
        }
        for chunk in response.chunks {
            output.push_str(&String::from_utf8_lossy(&chunk.chunk.into_inner()));
            after_seq = Some(chunk.seq);
        }
        if response.exited {
            exit_code = response.exit_code;
        }
        if response.closed {
            break;
        }
        after_seq = response.next_seq.checked_sub(1).or(after_seq);
    }
    drop(session);
    Ok((output, exit_code, true))
}

async fn collect_process_output_from_events(
    session: Arc<dyn ExecProcess>,
) -> Result<(String, String, Option<i32>, bool)> {
    let mut events = session.subscribe_events();
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = None;
    loop {
        match timeout(Duration::from_secs(2), events.recv()).await?? {
            ExecProcessEvent::Output(chunk) => match chunk.stream {
                ExecOutputStream::Stdout | ExecOutputStream::Pty => {
                    stdout.push_str(&String::from_utf8_lossy(&chunk.chunk.into_inner()));
                }
                ExecOutputStream::Stderr => {
                    stderr.push_str(&String::from_utf8_lossy(&chunk.chunk.into_inner()));
                }
            },
            ExecProcessEvent::Exited {
                seq: _,
                exit_code: code,
            } => {
                exit_code = Some(code);
            }
            ExecProcessEvent::Closed { seq: _ } => {
                drop(session);
                return Ok((stdout, stderr, exit_code, true));
            }
            ExecProcessEvent::Failed(message) => {
                anyhow::bail!("process failed before closed state: {message}");
            }
        }
    }
}

async fn collect_process_event_snapshots(
    session: Arc<dyn ExecProcess>,
) -> Result<Vec<ProcessEventSnapshot>> {
    let mut events = session.subscribe_events();
    let mut snapshots = Vec::new();
    loop {
        let snapshot = match timeout(Duration::from_secs(2), events.recv()).await?? {
            ExecProcessEvent::Output(chunk) => ProcessEventSnapshot::Output {
                seq: chunk.seq,
                stream: chunk.stream,
                text: String::from_utf8_lossy(&chunk.chunk.into_inner()).into_owned(),
            },
            ExecProcessEvent::Exited { seq, exit_code } => {
                ProcessEventSnapshot::Exited { seq, exit_code }
            }
            ExecProcessEvent::Closed { seq } => ProcessEventSnapshot::Closed { seq },
            ExecProcessEvent::Failed(message) => {
                anyhow::bail!("process failed before closed state: {message}");
            }
        };
        let closed = matches!(snapshot, ProcessEventSnapshot::Closed { .. });
        snapshots.push(snapshot);
        if closed {
            drop(session);
            return Ok(snapshots);
        }
    }
}

async fn assert_exec_process_streams_output(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-stream".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "sleep 0.05; printf 'session output\\n'".to_string(),
            ],
            cwd: std::env::current_dir()?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let StartedExecProcess { process } = session;
    let wake_rx = process.subscribe_wake();
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;
    assert_eq!(output, "session output\n");
    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

async fn assert_exec_process_pushes_events(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-events".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "printf 'event output\\n'; sleep 0.1; printf 'event err\\n' >&2; sleep 0.1; exit 7".to_string(),
            ],
            cwd: std::env::current_dir()?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let StartedExecProcess { process } = session;
    let actual = collect_process_event_snapshots(process).await?;
    assert_eq!(
        actual,
        vec![
            ProcessEventSnapshot::Output {
                seq: 1,
                stream: ExecOutputStream::Stdout,
                text: "event output\n".to_string(),
            },
            ProcessEventSnapshot::Output {
                seq: 2,
                stream: ExecOutputStream::Stderr,
                text: "event err\n".to_string(),
            },
            ProcessEventSnapshot::Exited {
                seq: 3,
                exit_code: 7,
            },
            ProcessEventSnapshot::Closed { seq: 4 },
        ]
    );
    Ok(())
}

async fn assert_exec_process_replays_events_after_close(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-events-late".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "printf 'late one\\n'; printf 'late two\\n'".to_string(),
            ],
            cwd: std::env::current_dir()?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let StartedExecProcess { process } = session;
    let wake_rx = process.subscribe_wake();
    let read_result = collect_process_output_from_reads(Arc::clone(&process), wake_rx).await?;
    assert_eq!(
        read_result,
        ("late one\nlate two\n".to_string(), Some(0), true)
    );

    let event_result = collect_process_output_from_events(process).await?;
    assert_eq!(
        event_result,
        (
            "late one\nlate two\n".to_string(),
            String::new(),
            Some(0),
            true
        )
    );
    Ok(())
}

async fn assert_exec_process_write_then_read(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-stdin".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone().into(),
            argv: vec![
                // Use `/bin/sh` instead of Python so this stdin round-trip test
                // stays portable across Bazel and non-macOS runners where
                // `/usr/bin/python3` is not guaranteed to exist.
                "/bin/sh".to_string(),
                "-c".to_string(),
                "IFS= read line; printf 'from-stdin:%s\\n' \"$line\"".to_string(),
            ],
            cwd: std::env::current_dir()?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: true,
            pipe_stdin: false,
            arg0: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    tokio::time::sleep(Duration::from_millis(200)).await;
    session.process.write(b"hello\n".to_vec()).await?;
    let StartedExecProcess { process } = session;
    let wake_rx = process.subscribe_wake();
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;

    assert!(
        output.contains("from-stdin:hello"),
        "unexpected output: {output:?}"
    );
    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

async fn assert_exec_process_write_then_read_without_tty(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-stdin-pipe".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "IFS= read line; printf 'from-stdin:%s\\n' \"$line\"".to_string(),
            ],
            cwd: std::env::current_dir()?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: true,
            arg0: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    tokio::time::sleep(Duration::from_millis(200)).await;
    let write_response = session.process.write(b"hello\n".to_vec()).await?;
    assert_eq!(write_response.status, WriteStatus::Accepted);
    let StartedExecProcess { process } = session;
    let wake_rx = process.subscribe_wake();
    let actual = collect_process_output_from_reads(process, wake_rx).await?;

    assert_eq!(actual, ("from-stdin:hello\n".to_string(), Some(0), true));
    Ok(())
}

async fn assert_exec_process_rejects_write_without_pipe_stdin(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-stdin-closed".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone().into(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "sleep 0.3; if IFS= read -r line; then printf 'read:%s\\n' \"$line\"; else printf 'eof\\n'; fi".to_string(),
            ],
            cwd: std::env::current_dir()?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let write_response = session.process.write(b"ignored\n".to_vec()).await?;
    assert_eq!(write_response.status, WriteStatus::StdinClosed);
    let StartedExecProcess { process } = session;
    let wake_rx = process.subscribe_wake();
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;

    assert_eq!(output, "eof\n");
    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

async fn assert_exec_process_preserves_queued_events_before_subscribe(
    use_remote: bool,
) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let session = context
        .backend
        .start(ExecParams {
            process_id: ProcessId::from("proc-queued"),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "printf 'queued output\\n'".to_string(),
            ],
            cwd: std::env::current_dir()?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
        })
        .await?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let StartedExecProcess { process } = session;
    let wake_rx = process.subscribe_wake();
    let (output, exit_code, closed) = collect_process_output_from_reads(process, wake_rx).await?;
    assert_eq!(output, "queued output\n");
    assert_eq!(exit_code, Some(0));
    assert!(closed);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn remote_exec_process_reports_transport_disconnect() -> Result<()> {
    let mut context = create_process_context(/*use_remote*/ true).await?;
    let session = context
        .backend
        .start(ExecParams {
            process_id: ProcessId::from("proc-disconnect"),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "sleep 10".to_string(),
            ],
            cwd: std::env::current_dir()?,
            env_policy: /*env_policy*/ None,
            env: Default::default(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
        })
        .await?;

    let process = Arc::clone(&session.process);
    let mut events = process.subscribe_events();
    let server = context
        .server
        .as_mut()
        .expect("remote context should include exec-server harness");
    server.shutdown().await?;

    let event = timeout(Duration::from_secs(2), events.recv()).await??;
    let ExecProcessEvent::Failed(event_message) = event else {
        anyhow::bail!("expected process failure event, got {event:?}");
    };
    assert!(
        event_message.starts_with("exec-server transport disconnected"),
        "unexpected failure event: {event_message}"
    );

    let mut wake_rx = process.subscribe_wake();
    let response = read_process_until_change(process, &mut wake_rx, /*after_seq*/ None).await?;
    let message = response
        .failure
        .expect("disconnect should surface as a failure");
    assert!(
        message.starts_with("exec-server transport disconnected"),
        "unexpected failure message: {message}"
    );
    assert!(
        response.closed,
        "disconnect should close the process session"
    );

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_starts_and_exits(use_remote: bool) -> Result<()> {
    assert_exec_process_starts_and_exits(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_streams_output(use_remote: bool) -> Result<()> {
    assert_exec_process_streams_output(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_pushes_events(use_remote: bool) -> Result<()> {
    assert_exec_process_pushes_events(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_replays_events_after_close(use_remote: bool) -> Result<()> {
    assert_exec_process_replays_events_after_close(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_write_then_read(use_remote: bool) -> Result<()> {
    assert_exec_process_write_then_read(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_write_then_read_without_tty(use_remote: bool) -> Result<()> {
    assert_exec_process_write_then_read_without_tty(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_rejects_write_without_pipe_stdin(use_remote: bool) -> Result<()> {
    assert_exec_process_rejects_write_without_pipe_stdin(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// Serialize tests that launch a real exec-server process through the full CLI.
#[serial_test::serial(remote_exec_server)]
async fn exec_process_preserves_queued_events_before_subscribe(use_remote: bool) -> Result<()> {
    assert_exec_process_preserves_queued_events_before_subscribe(use_remote).await
}
