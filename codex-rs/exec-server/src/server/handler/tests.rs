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
use crate::protocol::ReadResponse;
use crate::protocol::TerminateParams;
use crate::protocol::TerminateResponse;
use crate::rpc::RpcNotificationSender;
use crate::server::session_registry::SessionRegistry;

fn exec_params(process_id: &str) -> ExecParams {
    exec_params_with_argv(process_id, sleep_argv())
}

fn exec_params_with_argv(process_id: &str, argv: Vec<String>) -> ExecParams {
    ExecParams {
        process_id: ProcessId::from(process_id),
        argv,
        cwd: std::env::current_dir().expect("cwd"),
        env: inherited_path_env(),
        tty: false,
        arg0: None,
    }
}

fn inherited_path_env() -> HashMap<String, String> {
    let mut env = HashMap::new();
    if let Some(path) = std::env::var_os("PATH") {
        env.insert("PATH".to_string(), path.to_string_lossy().into_owned());
    }
    env
}

fn sleep_argv() -> Vec<String> {
    shell_argv("sleep 0.1", "ping -n 2 127.0.0.1 >NUL")
}

fn shell_argv(unix_script: &str, windows_script: &str) -> Vec<String> {
    if cfg!(windows) {
        vec![
            windows_command_processor(),
            "/C".to_string(),
            windows_script.to_string(),
        ]
    } else {
        vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            unix_script.to_string(),
        ]
    }
}

fn windows_command_processor() -> String {
    std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
}

async fn initialized_handler() -> Arc<ExecServerHandler> {
    let (outgoing_tx, _outgoing_rx) = mpsc::channel(16);
    let registry = SessionRegistry::new();
    let handler = Arc::new(ExecServerHandler::new(
        registry,
        RpcNotificationSender::new(outgoing_tx),
    ));
    let initialize_response = handler
        .initialize(InitializeParams {
            client_name: "exec-server-test".to_string(),
            resume_session_id: None,
        })
        .await
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
    let registry = SessionRegistry::new();
    let first_handler = Arc::new(ExecServerHandler::new(
        Arc::clone(&registry),
        RpcNotificationSender::new(first_tx),
    ));
    let initialize_response = first_handler
        .initialize(InitializeParams {
            client_name: "exec-server-test".to_string(),
            resume_session_id: None,
        })
        .await
        .expect("initialize");
    first_handler.initialized().expect("initialized");

    first_handler
        .exec(exec_params_with_argv(
            "proc-long-poll",
            shell_argv(
                "sleep 0.1; printf resumed",
                "ping -n 2 127.0.0.1 >NUL && echo resumed",
            ),
        ))
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
    first_handler.shutdown().await;

    let (second_tx, _second_rx) = mpsc::channel(16);
    let second_handler = Arc::new(ExecServerHandler::new(
        registry,
        RpcNotificationSender::new(second_tx),
    ));
    second_handler
        .initialize(InitializeParams {
            client_name: "exec-server-test".to_string(),
            resume_session_id: Some(initialize_response.session_id),
        })
        .await
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

#[tokio::test]
async fn active_session_resume_is_rejected() {
    let (first_tx, _first_rx) = mpsc::channel(16);
    let registry = SessionRegistry::new();
    let first_handler = Arc::new(ExecServerHandler::new(
        Arc::clone(&registry),
        RpcNotificationSender::new(first_tx),
    ));
    let initialize_response = first_handler
        .initialize(InitializeParams {
            client_name: "exec-server-test".to_string(),
            resume_session_id: None,
        })
        .await
        .expect("initialize");

    let (second_tx, _second_rx) = mpsc::channel(16);
    let second_handler = Arc::new(ExecServerHandler::new(
        registry,
        RpcNotificationSender::new(second_tx),
    ));
    let err = second_handler
        .initialize(InitializeParams {
            client_name: "exec-server-test".to_string(),
            resume_session_id: Some(initialize_response.session_id.clone()),
        })
        .await
        .expect_err("active session resume should fail");

    assert_eq!(err.code, -32600);
    assert_eq!(
        err.message,
        format!(
            "session {} is already attached to another connection",
            initialize_response.session_id
        )
    );

    first_handler.shutdown().await;
}

#[tokio::test]
async fn output_and_exit_are_retained_after_notification_receiver_closes() {
    let (outgoing_tx, outgoing_rx) = mpsc::channel(16);
    let handler = Arc::new(ExecServerHandler::new(
        SessionRegistry::new(),
        RpcNotificationSender::new(outgoing_tx),
    ));
    handler
        .initialize(InitializeParams {
            client_name: "exec-server-test".to_string(),
            resume_session_id: None,
        })
        .await
        .expect("initialize");
    handler.initialized().expect("initialized");

    let process_id = ProcessId::from("proc-notification-fail");
    handler
        .exec(exec_params_with_argv(
            process_id.as_str(),
            shell_argv(
                "sleep 0.05; printf 'first\\n'; sleep 0.05; printf 'second\\n'",
                "(echo first) && ping -n 2 127.0.0.1 >NUL && (echo second)",
            ),
        ))
        .await
        .expect("start process");

    drop(outgoing_rx);

    let (output, exit_code) = read_process_until_closed(&handler, process_id.clone()).await;
    assert_eq!(output.replace("\r\n", "\n"), "first\nsecond\n");
    assert_eq!(exit_code, Some(0));

    tokio::time::sleep(Duration::from_millis(100)).await;
    handler
        .exec(exec_params(process_id.as_str()))
        .await
        .expect("process id should be reusable after exit retention");

    handler.shutdown().await;
}

async fn read_process_until_closed(
    handler: &ExecServerHandler,
    process_id: ProcessId,
) -> (String, Option<i32>) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let mut output = String::new();
    let mut exit_code = None;
    let mut after_seq = None;

    loop {
        let response: ReadResponse = handler
            .exec_read(ReadParams {
                process_id: process_id.clone(),
                after_seq,
                max_bytes: None,
                wait_ms: Some(500),
            })
            .await
            .expect("read process");

        for chunk in response.chunks {
            output.push_str(&String::from_utf8_lossy(&chunk.chunk.into_inner()));
            after_seq = Some(chunk.seq);
        }
        if response.exited {
            exit_code = response.exit_code;
        }
        if response.closed {
            return (output, exit_code);
        }
        after_seq = response.next_seq.checked_sub(1).or(after_seq);
        assert!(
            tokio::time::Instant::now() < deadline,
            "process should close within 2s"
        );
    }
}
