#![cfg(unix)]

mod common;

use std::sync::Arc;

use anyhow::Result;
use codex_exec_server::Environment;
use codex_exec_server::ExecBackend;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcess;
use codex_exec_server::ExecSessionEvent;
use codex_exec_server::StartedExecProcess;
use pretty_assertions::assert_eq;
use test_case::test_case;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio::time::timeout;

use common::exec_server::ExecServerHarness;
use common::exec_server::exec_server;

struct ProcessContext {
    backend: Arc<dyn ExecBackend>,
    server: Option<ExecServerHarness>,
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
        let environment = Environment::create(None).await?;
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
            process_id: "proc-1".to_string(),
            argv: vec!["true".to_string()],
            cwd: std::env::current_dir()?,
            env: Default::default(),
            tty: false,
            arg0: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), "proc-1");
    let mut events = session.events;

    let mut exit_code = None;
    loop {
        match timeout(Duration::from_secs(2), events.recv()).await? {
            Some(event) => match event {
                ExecSessionEvent::Exited {
                    exit_code: code, ..
                } => exit_code = Some(code),
                ExecSessionEvent::Closed { .. } => break,
                ExecSessionEvent::Output { .. } => {}
                ExecSessionEvent::Failed { message } => {
                    anyhow::bail!("process failed before Closed event: {message}")
                }
            },
            None => anyhow::bail!("event stream closed before Closed event"),
        }
    }

    assert_eq!(exit_code, Some(0));
    Ok(())
}

async fn collect_process_output_from_events(
    session: Arc<dyn ExecProcess>,
    mut events: mpsc::Receiver<ExecSessionEvent>,
) -> Result<(String, i32, bool)> {
    let mut output = String::new();
    let mut exit_code = None;
    loop {
        match timeout(Duration::from_secs(2), events.recv()).await? {
            Some(event) => match event {
                ExecSessionEvent::Output { chunk, .. } => {
                    output.push_str(&String::from_utf8_lossy(&chunk));
                }
                ExecSessionEvent::Exited {
                    exit_code: code, ..
                } => exit_code = Some(code),
                ExecSessionEvent::Closed { .. } => {
                    break;
                }
                ExecSessionEvent::Failed { message } => {
                    anyhow::bail!("process failed before Closed event: {message}");
                }
            },
            None => {
                anyhow::bail!("event stream closed before Closed event");
            }
        }
    }
    drop(session);
    Ok((output, exit_code.unwrap_or(-1), true))
}

async fn assert_exec_process_streams_output(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-stream".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "sleep 0.05; printf 'session output\\n'".to_string(),
            ],
            cwd: std::env::current_dir()?,
            env: Default::default(),
            tty: false,
            arg0: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    let StartedExecProcess { process, events } = session;
    let (output, exit_code, closed) = collect_process_output_from_events(process, events).await?;
    assert_eq!(output, "session output\n");
    assert_eq!(exit_code, 0);
    assert!(closed);
    Ok(())
}

async fn assert_exec_process_write_then_read(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-stdin".to_string();
    let session = context
        .backend
        .start(ExecParams {
            process_id: process_id.clone(),
            argv: vec![
                "/usr/bin/python3".to_string(),
                "-c".to_string(),
                "import sys; line = sys.stdin.readline(); sys.stdout.write(f'from-stdin:{line}'); sys.stdout.flush()".to_string(),
            ],
            cwd: std::env::current_dir()?,
            env: Default::default(),
            tty: true,
            arg0: None,
        })
        .await?;
    assert_eq!(session.process.process_id().as_str(), process_id);

    tokio::time::sleep(Duration::from_millis(200)).await;
    session.process.write(b"hello\n".to_vec()).await?;
    let StartedExecProcess { process, events } = session;
    let (output, exit_code, closed) = collect_process_output_from_events(process, events).await?;

    assert!(
        output.contains("from-stdin:hello"),
        "unexpected output: {output:?}"
    );
    assert_eq!(exit_code, 0);
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
            process_id: "proc-queued".to_string(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "printf 'queued output\\n'".to_string(),
            ],
            cwd: std::env::current_dir()?,
            env: Default::default(),
            tty: false,
            arg0: None,
        })
        .await?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let StartedExecProcess { process, events } = session;
    let (output, exit_code, closed) = collect_process_output_from_events(process, events).await?;
    assert_eq!(output, "queued output\n");
    assert_eq!(exit_code, 0);
    assert!(closed);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_exec_process_reports_transport_disconnect() -> Result<()> {
    let mut context = create_process_context(/*use_remote*/ true).await?;
    let session = context
        .backend
        .start(ExecParams {
            process_id: "proc-disconnect".to_string(),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "sleep 10".to_string(),
            ],
            cwd: std::env::current_dir()?,
            env: Default::default(),
            tty: false,
            arg0: None,
        })
        .await?;

    let server = context
        .server
        .as_mut()
        .expect("remote context should include exec-server harness");
    server.shutdown().await?;

    let mut events = session.events;
    loop {
        match timeout(Duration::from_secs(2), events.recv()).await? {
            Some(ExecSessionEvent::Failed { message }) => {
                assert!(
                    message.starts_with("exec-server transport disconnected"),
                    "unexpected failure message: {message}"
                );
                break;
            }
            Some(ExecSessionEvent::Output { .. } | ExecSessionEvent::Exited { .. }) => {}
            Some(ExecSessionEvent::Closed { .. }) => {
                anyhow::bail!("received Closed instead of transport failure")
            }
            None => anyhow::bail!("event stream closed before Failed event"),
        }
    }

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_process_starts_and_exits(use_remote: bool) -> Result<()> {
    assert_exec_process_starts_and_exits(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_process_streams_output(use_remote: bool) -> Result<()> {
    assert_exec_process_streams_output(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_process_write_then_read(use_remote: bool) -> Result<()> {
    assert_exec_process_write_then_read(use_remote).await
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_process_preserves_queued_events_before_subscribe(use_remote: bool) -> Result<()> {
    assert_exec_process_preserves_queued_events_before_subscribe(use_remote).await
}
