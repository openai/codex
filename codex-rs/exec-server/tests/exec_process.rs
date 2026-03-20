#![cfg(unix)]

mod common;

use std::sync::Arc;

use anyhow::Result;
use codex_exec_server::Environment;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcess;
use codex_exec_server::ExecResponse;
use codex_exec_server::ReadParams;
use codex_exec_server::TerminateResponse;
use pretty_assertions::assert_eq;
use test_case::test_case;

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
    let response = context
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
    assert_eq!(
        response,
        ExecResponse {
            process_id: "proc-1".to_string(),
        }
    );

    let mut next_seq = 0;
    loop {
        let read = context
            .process
            .read(ReadParams {
                process_id: "proc-1".to_string(),
                after_seq: Some(next_seq),
                max_bytes: None,
                wait_ms: Some(100),
            })
            .await?;
        next_seq = read.next_seq;
        if read.exited {
            assert_eq!(read.exit_code, Some(0));
            break;
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

async fn assert_exec_process_terminates_running_process(use_remote: bool) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let response = context
        .process
        .start(ExecParams {
            process_id: "proc-io".to_string(),
            argv: vec!["sleep".to_string(), "60".to_string()],
            cwd: std::env::current_dir()?,
            env: Default::default(),
            tty: false,
            arg0: None,
        })
        .await?;
    assert_eq!(
        response,
        ExecResponse {
            process_id: "proc-io".to_string(),
        }
    );

    let terminate = context.process.terminate("proc-io").await?;
    assert_eq!(terminate, TerminateResponse { running: true });

    let mut next_seq = 0;
    loop {
        let read = context
            .process
            .read(ReadParams {
                process_id: "proc-io".to_string(),
                after_seq: Some(next_seq),
                max_bytes: None,
                wait_ms: Some(100),
            })
            .await?;
        next_seq = read.next_seq;
        if read.exited {
            break;
        }
    }

    let terminate_after_exit = context.process.terminate("proc-io").await?;
    assert_eq!(terminate_after_exit, TerminateResponse { running: false });

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_process_terminates_running_process(use_remote: bool) -> Result<()> {
    assert_exec_process_terminates_running_process(use_remote).await
}

async fn assert_exec_process_read_returns_one_chunk_when_max_bytes_is_zero(
    use_remote: bool,
) -> Result<()> {
    let context = create_process_context(use_remote).await?;
    let response = context
        .process
        .start(ExecParams {
            process_id: "proc-truncate".to_string(),
            argv: vec!["sh".to_string(), "-c".to_string(), "printf a".to_string()],
            cwd: std::env::current_dir()?,
            env: Default::default(),
            tty: false,
            arg0: None,
        })
        .await?;
    assert_eq!(
        response,
        ExecResponse {
            process_id: "proc-truncate".to_string(),
        }
    );

    let mut read = None;
    for _ in 0..20 {
        let candidate = context
            .process
            .read(ReadParams {
                process_id: "proc-truncate".to_string(),
                after_seq: Some(0),
                max_bytes: Some(0),
                wait_ms: Some(100),
            })
            .await?;
        if !candidate.chunks.is_empty() {
            read = Some(candidate);
            break;
        }
    }
    let Some(read) = read else {
        anyhow::bail!("timed out waiting for retained output with max_bytes = 0");
    };
    assert_eq!(read.chunks.len(), 1);
    assert_eq!(read.chunks[0].chunk.0, b"a");
    assert_eq!(read.next_seq, 2);

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_process_read_returns_one_chunk_when_max_bytes_is_zero(
    use_remote: bool,
) -> Result<()> {
    assert_exec_process_read_returns_one_chunk_when_max_bytes_is_zero(use_remote).await
}
