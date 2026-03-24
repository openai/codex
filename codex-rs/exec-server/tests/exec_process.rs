#![cfg(unix)]

mod common;

use std::sync::Arc;

use anyhow::Result;
use codex_exec_server::Environment;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcess;
use codex_exec_server::ExecSessionEvent;
use codex_exec_server::ExecSessionHandle;
use pretty_assertions::assert_eq;
use test_case::test_case;
use tokio::time::Duration;
use tokio::time::timeout;

use common::exec_server::ExecServerHarness;
use common::exec_server::exec_server;

struct ProcessContext {
    process: Arc<dyn ExecProcess>,
    _server: Option<ExecServerHarness>,
}

async fn create_process_context(use_remote: bool) -> Result<ProcessContext> {
    if use_remote {
        let server = exec_server().await?;
        let environment = Environment::create(Some(server.websocket_url().to_string())).await?;
        Ok(ProcessContext {
            process: environment.get_executor(),
            _server: Some(server),
        })
    } else {
        let environment = Environment::create(None).await?;
        Ok(ProcessContext {
            process: environment.get_executor(),
            _server: None,
        })
    }
}

async fn assert_exec_process_starts_and_exits(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let mut session = context
        .process
        .start(ExecParams {
            process_id: "proc-1".to_string(),
            argv: vec!["true".to_string()],
            cwd: std::env::current_dir()?,
            env: Default::default(),
            tty: false,
            arg0: None,
        })
        .await?;
    assert_eq!(session.process_id, "proc-1");

    let mut exit_code = None;
    loop {
        match timeout(Duration::from_secs(2), session.events.recv()).await?? {
            ExecSessionEvent::Exited {
                exit_code: code, ..
            } => exit_code = Some(code),
            ExecSessionEvent::Closed { .. } => break,
            ExecSessionEvent::Output { .. } => {}
        }
    }

    assert_eq!(exit_code, Some(0));
    Ok(())
}

async fn collect_process_output_from_events(
    mut session: ExecSessionHandle,
) -> Result<(String, i32, bool)> {
    let mut output = String::new();
    let mut exit_code = None;
    loop {
        match timeout(Duration::from_secs(2), session.events.recv()).await?? {
            ExecSessionEvent::Output { chunk, .. } => {
                output.push_str(&String::from_utf8_lossy(&chunk));
            }
            ExecSessionEvent::Exited {
                exit_code: code, ..
            } => exit_code = Some(code),
            ExecSessionEvent::Closed { .. } => {
                break;
            }
        }
    }
    Ok((output, exit_code.unwrap_or(-1), true))
}

async fn assert_exec_process_streams_output(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-stream".to_string();
    let session = context
        .process
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
    assert_eq!(session.process_id, process_id);

    let (output, exit_code, closed) = collect_process_output_from_events(session).await?;
    assert_eq!(output, "session output\n");
    assert_eq!(exit_code, 0);
    assert!(closed);
    Ok(())
}

async fn assert_exec_process_write_then_read(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let process_id = "proc-stdin".to_string();
    let session = context
        .process
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
    assert_eq!(session.process_id, process_id);

    tokio::time::sleep(Duration::from_millis(200)).await;
    session.write_stdin(b"hello\n".to_vec()).await?;
    let (output, exit_code, closed) = collect_process_output_from_events(session).await?;

    assert!(
        output.contains("from-stdin:hello"),
        "unexpected output: {output:?}"
    );
    assert_eq!(exit_code, 0);
    assert!(closed);
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
