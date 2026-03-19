#![cfg(unix)]

mod common;

use std::sync::Arc;

use anyhow::Result;
use codex_exec_server::Environment;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcess;
use codex_exec_server::ExecResponse;
use codex_exec_server::ReadParams;
use pretty_assertions::assert_eq;

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_process_starts_and_exits_locally() -> Result<()> {
    assert_exec_process_starts_and_exits(false).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_process_starts_and_exits_remotely() -> Result<()> {
    assert_exec_process_starts_and_exits(true).await
}
