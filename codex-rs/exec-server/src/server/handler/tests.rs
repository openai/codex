use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use pretty_assertions::assert_eq;
use tokio::sync::mpsc;

use super::ExecServerHandler;
use crate::protocol::ExecParams;
use crate::protocol::InitializeResponse;
use crate::rpc::RpcNotificationSender;

fn exec_params(process_id: &str) -> ExecParams {
    let mut env = HashMap::new();
    if let Some(path) = std::env::var_os("PATH") {
        env.insert("PATH".to_string(), path.to_string_lossy().into_owned());
    }
    ExecParams {
        process_id: process_id.to_string(),
        argv: vec![
            "bash".to_string(),
            "-lc".to_string(),
            "sleep 0.1".to_string(),
        ],
        cwd: std::env::current_dir().expect("cwd"),
        env,
        tty: false,
        arg0: None,
    }
}

async fn initialized_handler() -> Arc<ExecServerHandler> {
    let (outgoing_tx, _outgoing_rx) = mpsc::channel(16);
    let handler = Arc::new(ExecServerHandler::new(RpcNotificationSender::new(
        outgoing_tx,
    )));
    assert_eq!(
        handler.initialize().expect("initialize"),
        InitializeResponse {}
    );
    handler.initialized().expect("initialized");
    handler
}

#[tokio::test]
async fn duplicate_process_ids_allow_only_one_successful_start() {
    let handler = initialized_handler().await;
    let first_handler = Arc::clone(&handler);
    let second_handler = Arc::clone(&handler);

    let (first, second) = tokio::join!(
        first_handler.exec(exec_params("proc-1")),
        second_handler.exec(exec_params("proc-1")),
    );

    let (successes, failures): (Vec<_>, Vec<_>) =
        [first, second].into_iter().partition(Result::is_ok);
    assert_eq!(successes.len(), 1);
    assert_eq!(failures.len(), 1);

    let error = failures
        .into_iter()
        .next()
        .expect("one failed request")
        .expect_err("expected duplicate process error");
    assert_eq!(error.code, -32600);
    assert_eq!(error.message, "process proc-1 already exists");

    tokio::time::sleep(Duration::from_millis(150)).await;
    handler.shutdown().await;
}
