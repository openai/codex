use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use pretty_assertions::assert_eq;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::ExecServerHandler;
use crate::ProcessId;
use crate::protocol::ExecParams;
use crate::protocol::InitializeParams;
use crate::protocol::ReadParams;
use crate::protocol::TerminateParams;
use crate::protocol::TerminateResponse;
use crate::rpc::RpcNotificationSender;
use crate::server::session_registry::SessionRegistry;

fn exec_params(process_id: &str) -> ExecParams {
    let mut env = HashMap::new();
    if let Some(path) = std::env::var_os("PATH") {
        env.insert("PATH".to_string(), path.to_string_lossy().into_owned());
    }
    ExecParams {
        process_id: ProcessId::from(process_id),
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
    let registry = SessionRegistry::default();
    let session = registry
        .attach(
            /*resume_session_id*/ None,
            RpcNotificationSender::new(outgoing_tx),
        )
        .await
        .expect("attach session");
    let handler = Arc::new(ExecServerHandler::new(session));
    let initialize_response = handler
        .initialize(InitializeParams {
            client_name: "exec-server-test".to_string(),
            resume_session_id: None,
        })
        .expect("initialize");
    Uuid::parse_str(&initialize_response.session_id).expect("session id should be a UUID");
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

#[tokio::test]
async fn terminate_reports_false_after_process_exit() {
    let handler = initialized_handler().await;
    handler
        .exec(exec_params("proc-1"))
        .await
        .expect("start process");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    loop {
        let response = handler
            .terminate(TerminateParams {
                process_id: ProcessId::from("proc-1"),
            })
            .await
            .expect("terminate response");
        if response == (TerminateResponse { running: false }) {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "process should have exited within 1s"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    handler.shutdown().await;
}

#[tokio::test]
async fn long_poll_read_fails_after_session_resume() {
    let (first_tx, _first_rx) = mpsc::channel(16);
    let registry = SessionRegistry::default();
    let first_session = registry
        .attach(
            /*resume_session_id*/ None,
            RpcNotificationSender::new(first_tx),
        )
        .await
        .expect("attach first session");
    let first_handler = Arc::new(ExecServerHandler::new(first_session));
    let initialize_response = first_handler
        .initialize(InitializeParams {
            client_name: "exec-server-test".to_string(),
            resume_session_id: None,
        })
        .expect("initialize");
    first_handler.initialized().expect("initialized");

    first_handler
        .exec(ExecParams {
            process_id: ProcessId::from("proc-long-poll"),
            argv: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "sleep 0.1; printf resumed".to_string(),
            ],
            cwd: std::env::current_dir().expect("cwd"),
            env: HashMap::new(),
            tty: false,
            arg0: None,
        })
        .await
        .expect("start process");

    let first_read_handler = Arc::clone(&first_handler);
    let read_task = tokio::spawn(async move {
        first_read_handler
            .exec_read(ReadParams {
                process_id: ProcessId::from("proc-long-poll"),
                after_seq: None,
                max_bytes: None,
                wait_ms: Some(500),
            })
            .await
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let (second_tx, _second_rx) = mpsc::channel(16);
    let second_session = registry
        .attach(
            Some(initialize_response.session_id),
            RpcNotificationSender::new(second_tx),
        )
        .await
        .expect("attach second session");
    let second_handler = Arc::new(ExecServerHandler::new(second_session));
    second_handler
        .initialize(InitializeParams {
            client_name: "exec-server-test".to_string(),
            resume_session_id: None,
        })
        .expect("initialize second connection");
    second_handler
        .initialized()
        .expect("initialized second connection");

    let err = read_task
        .await
        .expect("read task should join")
        .expect_err("evicted long-poll read should fail");
    assert_eq!(err.code, -32600);
    assert_eq!(
        err.message,
        "session has been resumed by another connection"
    );

    second_handler.shutdown().await;
}
