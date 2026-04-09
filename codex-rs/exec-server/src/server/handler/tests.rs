use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use pretty_assertions::assert_eq;
use tokio::sync::mpsc;

use super::ExecServerHandler;
use crate::ProcessId;
use crate::protocol::ExecParams;
use crate::protocol::InitializeResponse;
use crate::protocol::ReadParams;
use crate::protocol::ResolveExecApprovalParams;
use crate::protocol::TerminateParams;
use crate::protocol::TerminateResponse;
use crate::rpc::RpcNotificationSender;
use codex_app_server_protocol::CommandExecutionApprovalDecision;

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
        startup_exec_approval: None,
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
async fn startup_exec_approval_spawns_only_after_resolution() {
    let handler = initialized_handler().await;
    let mut params = exec_params("proc-approval");
    params.argv = vec![
        "bash".to_string(),
        "-lc".to_string(),
        "printf ready".to_string(),
    ];
    params.startup_exec_approval = Some(crate::protocol::ExecApprovalRequest {
        call_id: "call-1".to_string(),
        approval_id: None,
        turn_id: "turn-1".to_string(),
        command: vec![
            "bash".to_string(),
            "-lc".to_string(),
            "printf ready".to_string(),
        ],
        cwd: std::env::current_dir().expect("cwd"),
        reason: Some("approval required".to_string()),
        additional_permissions: None,
        proposed_execpolicy_amendment: None,
        available_decisions: Some(vec![CommandExecutionApprovalDecision::Accept]),
    });
    handler.exec(params).await.expect("start process");

    let pending = handler
        .exec_read(ReadParams {
            process_id: ProcessId::from("proc-approval"),
            after_seq: None,
            max_bytes: None,
            wait_ms: Some(0),
        })
        .await
        .expect("read pending approval");
    assert_eq!(pending.chunks, Vec::new());
    assert!(pending.exec_approval.is_some());

    handler
        .resolve_exec_approval(ResolveExecApprovalParams {
            process_id: ProcessId::from("proc-approval"),
            approval_id: "call-1".to_string(),
            decision: CommandExecutionApprovalDecision::Accept,
        })
        .await
        .expect("resolve approval");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    loop {
        let response = handler
            .exec_read(ReadParams {
                process_id: ProcessId::from("proc-approval"),
                after_seq: None,
                max_bytes: None,
                wait_ms: Some(0),
            })
            .await
            .expect("read process output");
        let output = response
            .chunks
            .iter()
            .flat_map(|chunk| chunk.chunk.0.iter().copied())
            .collect::<Vec<_>>();
        if String::from_utf8_lossy(&output).contains("ready") {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "approved process did not produce output within timeout"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    handler.shutdown().await;
}

#[tokio::test]
async fn startup_exec_approval_decline_returns_failure_without_spawning() {
    let handler = initialized_handler().await;
    let mut params = exec_params("proc-decline");
    params.argv = vec![
        "bash".to_string(),
        "-lc".to_string(),
        "printf should-not-run".to_string(),
    ];
    params.startup_exec_approval = Some(crate::protocol::ExecApprovalRequest {
        call_id: "call-2".to_string(),
        approval_id: None,
        turn_id: "turn-2".to_string(),
        command: vec![
            "bash".to_string(),
            "-lc".to_string(),
            "printf should-not-run".to_string(),
        ],
        cwd: std::env::current_dir().expect("cwd"),
        reason: Some("approval required".to_string()),
        additional_permissions: None,
        proposed_execpolicy_amendment: None,
        available_decisions: Some(vec![CommandExecutionApprovalDecision::Decline]),
    });
    handler.exec(params).await.expect("start process");

    handler
        .resolve_exec_approval(ResolveExecApprovalParams {
            process_id: ProcessId::from("proc-decline"),
            approval_id: "call-2".to_string(),
            decision: CommandExecutionApprovalDecision::Decline,
        })
        .await
        .expect("resolve approval");

    let response = handler
        .exec_read(ReadParams {
            process_id: ProcessId::from("proc-decline"),
            after_seq: None,
            max_bytes: None,
            wait_ms: Some(0),
        })
        .await
        .expect("read declined process");
    assert_eq!(response.chunks, Vec::new());
    assert_eq!(response.failure.as_deref(), Some("rejected by user"));
    assert!(response.closed);

    handler.shutdown().await;
}

#[tokio::test]
async fn startup_exec_approval_terminate_cancels_pending_start() {
    let handler = initialized_handler().await;
    let mut params = exec_params("proc-terminated");
    params.argv = vec![
        "bash".to_string(),
        "-lc".to_string(),
        "printf should-not-run".to_string(),
    ];
    params.startup_exec_approval = Some(crate::protocol::ExecApprovalRequest {
        call_id: "call-terminate".to_string(),
        approval_id: None,
        turn_id: "turn-terminate".to_string(),
        command: vec![
            "bash".to_string(),
            "-lc".to_string(),
            "printf should-not-run".to_string(),
        ],
        cwd: std::env::current_dir().expect("cwd"),
        reason: Some("approval required".to_string()),
        additional_permissions: None,
        proposed_execpolicy_amendment: None,
        available_decisions: Some(vec![CommandExecutionApprovalDecision::Accept]),
    });
    handler.exec(params).await.expect("start process");

    let pending = handler
        .exec_read(ReadParams {
            process_id: ProcessId::from("proc-terminated"),
            after_seq: None,
            max_bytes: None,
            wait_ms: Some(0),
        })
        .await
        .expect("read pending approval");
    assert!(pending.exec_approval.is_some());

    assert_eq!(
        handler
            .terminate(TerminateParams {
                process_id: ProcessId::from("proc-terminated"),
            })
            .await
            .expect("terminate response"),
        TerminateResponse { running: true }
    );

    let cancelled = handler
        .exec_read(ReadParams {
            process_id: ProcessId::from("proc-terminated"),
            after_seq: None,
            max_bytes: None,
            wait_ms: Some(0),
        })
        .await
        .expect("read cancelled process");
    assert_eq!(cancelled.chunks, Vec::new());
    assert_eq!(
        cancelled.failure.as_deref(),
        Some("terminated before process start")
    );
    assert!(cancelled.exec_approval.is_none());
    assert!(cancelled.closed);
    assert_eq!(cancelled.exit_code, Some(1));

    let error = handler
        .resolve_exec_approval(ResolveExecApprovalParams {
            process_id: ProcessId::from("proc-terminated"),
            approval_id: "call-terminate".to_string(),
            decision: CommandExecutionApprovalDecision::Accept,
        })
        .await
        .expect_err("terminated process should not accept approval");
    assert_eq!(error.code, -32600);
    assert_eq!(
        error.message,
        "process id proc-terminated has no pending exec approval"
    );

    assert_eq!(
        handler
            .terminate(TerminateParams {
                process_id: ProcessId::from("proc-terminated"),
            })
            .await
            .expect("second terminate response"),
        TerminateResponse { running: false }
    );

    handler.shutdown().await;
}
