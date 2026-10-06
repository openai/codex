use super::*;
use crate::thread_manager::default_thread_id_generator;
use codex_protocol::error::CodexErrorDetails;
use codex_thread_store::ThreadStoreError;
use futures::future;
use pretty_assertions::assert_eq;
use tokio::sync::Barrier;
use tokio::sync::oneshot;

fn runtime() -> LocalAgentRuntime {
    LocalAgentRuntime::new(
        Weak::default(),
        default_thread_id_generator(),
        /*rollout_budget*/ None,
    )
}

#[tokio::test]
async fn aborted_teardown_reports_operation_and_thread() {
    let runtime = runtime();
    let thread_id = ThreadId::new();
    let teardown = runtime
        .admit_start()
        .expect("teardown should be admitted")
        .into_teardown_guard("session_startup", Some(thread_id));
    let (ready_tx, ready_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        let _teardown = teardown;
        let _ = ready_tx.send(());
        future::pending::<()>().await;
    });
    ready_rx.await.expect("teardown task should start");
    let shutdown = runtime.request_shutdown();

    task.abort();
    task.await.expect_err("teardown task should be aborted");

    let report = shutdown
        .wait_detailed()
        .await
        .expect_err("aborted teardown must fail tree shutdown");
    assert_eq!(
        report,
        AgentTreeShutdownReport {
            tree_id: report.tree_id,
            failures: vec![AgentTreeShutdownFailure {
                operation: "session_startup",
                thread_id: Some(thread_id),
                reason: AgentTreeShutdownFailureReason::GuardAbandoned,
            }],
            omitted_failures: 0,
        }
    );
    assert_eq!(
        report.to_string(),
        format!(
            "agent tree shutdown did not complete cleanly: tree_id={}, failures=[{{operation=session_startup, thread_id={thread_id}, reason=guard_abandoned}}]; omitted_failures=0",
            report.tree_id,
        )
    );
    let error = shutdown
        .wait()
        .await
        .expect_err("legacy wait must fail after an abandoned teardown");
    assert!(matches!(
        error.details(),
        CodexErrorDetails::Fatal(message) if message == "agent tree shutdown did not complete cleanly"
    ));
}

#[tokio::test]
async fn completed_teardown_does_not_fail_tree_shutdown() {
    let runtime = runtime();
    let teardown = runtime
        .admit_start()
        .expect("teardown should be admitted")
        .into_teardown_guard("session_loop", /*thread_id*/ None);
    let shutdown = runtime.request_shutdown();
    teardown.complete();

    assert_eq!(shutdown.wait_detailed().await, Ok(()));
    assert!(shutdown.wait().await.is_ok());
}

#[tokio::test]
async fn report_display_includes_safe_cause_without_raw_error_payload() {
    let runtime = runtime();
    let thread_id = ThreadId::new();
    let teardown = runtime
        .admit_start()
        .expect("teardown should be admitted")
        .into_teardown_guard("session_startup", Some(thread_id));
    let raw_payload = "sensitive thread-store error payload";
    let error = ThreadStoreError::Internal {
        message: raw_payload.to_owned(),
    };
    teardown.record_shutdown_failure(
        "discard_persistence",
        crate::thread_manager::thread_store_error_kind(&error),
    );
    let shutdown = runtime.request_shutdown();
    teardown.complete();

    let report = shutdown
        .wait_detailed()
        .await
        .expect_err("recorded failure must fail shutdown");
    let displayed = report.to_string();
    assert_eq!(
        displayed,
        format!(
            "agent tree shutdown did not complete cleanly: tree_id={}, failures=[{{operation=session_startup, phase=discard_persistence, thread_id={thread_id}, reason=operation_failed, error_kind=thread_store_internal}}]; omitted_failures=0",
            report.tree_id,
        )
    );
    assert!(!displayed.contains(raw_payload));
}

#[tokio::test]
async fn cloned_guard_reports_its_own_context_before_releasing_membership() {
    let runtime = runtime();
    let original = runtime
        .admit_start()
        .expect("teardown should be admitted")
        .into_teardown_guard("startup", /*thread_id*/ None);
    let mut session = original.clone_for_teardown("session_loop", /*thread_id*/ None);
    let thread_id = ThreadId::new();
    session.set_thread_id(thread_id);
    let shutdown = runtime.request_shutdown();
    original.complete();

    let wait = shutdown.wait_detailed();
    tokio::pin!(wait);
    assert!(futures::poll!(&mut wait).is_pending());
    drop(session);
    let report = wait
        .await
        .expect_err("abandoned session must fail shutdown");
    assert_eq!(
        report,
        AgentTreeShutdownReport {
            tree_id: report.tree_id,
            failures: vec![AgentTreeShutdownFailure {
                operation: "session_loop",
                thread_id: Some(thread_id),
                reason: AgentTreeShutdownFailureReason::GuardAbandoned,
            }],
            omitted_failures: 0,
        }
    );
}

#[tokio::test]
async fn concurrent_failures_are_retained_for_every_waiter() {
    let runtime = runtime();
    let barrier = Arc::new(Barrier::new(/*n*/ 3));
    let thread_ids = [
        ThreadId::from_u128(/*value*/ 1),
        ThreadId::from_u128(/*value*/ 2),
    ];
    let mut tasks = Vec::new();
    for thread_id in thread_ids {
        let teardown = runtime
            .admit_start()
            .expect("teardown should be admitted")
            .into_teardown_guard("child_spawn", Some(thread_id));
        let barrier = Arc::clone(&barrier);
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            teardown.record_shutdown_failure("discard_persistence", "thread_store_internal");
            teardown.complete();
        }));
    }
    let shutdown = runtime.request_shutdown();
    let first_waiter = {
        let shutdown = Arc::clone(&shutdown);
        tokio::spawn(async move { shutdown.wait_detailed().await })
    };
    let second_waiter = {
        let shutdown = Arc::clone(&shutdown);
        tokio::spawn(async move { shutdown.wait_detailed().await })
    };
    barrier.wait().await;
    for task in tasks {
        task.await.expect("cleanup task should finish");
    }
    let mut first = first_waiter
        .await
        .expect("first waiter should finish")
        .expect_err("recorded failures must fail shutdown");
    let second = second_waiter
        .await
        .expect("second waiter should finish")
        .expect_err("recorded failures must fail shutdown");
    assert_eq!(first, second);
    assert_eq!(shutdown.wait_detailed().await, Err(first.clone()));

    first
        .failures
        .sort_by_key(|failure| failure.thread_id.map(|thread_id| thread_id.to_string()));
    assert_eq!(
        first,
        AgentTreeShutdownReport {
            tree_id: first.tree_id,
            failures: thread_ids
                .into_iter()
                .map(|thread_id| AgentTreeShutdownFailure::operation_failed(
                    "child_spawn",
                    "discard_persistence",
                    Some(thread_id),
                    "thread_store_internal",
                ))
                .collect(),
            omitted_failures: 0,
        }
    );
}

#[tokio::test]
async fn failure_report_counts_omitted_failures() {
    let runtime = runtime();
    let teardown = runtime
        .admit_start()
        .expect("teardown should be admitted")
        .into_teardown_guard("startup", /*thread_id*/ None);
    let failure = AgentTreeShutdownFailure::operation_failed(
        "startup",
        "stop_session",
        /*thread_id*/ None,
        "internal",
    );
    for _ in 0..MAX_RETAINED_SHUTDOWN_FAILURES + 3 {
        runtime.record_shutdown_failure(failure.clone());
    }
    let shutdown = runtime.request_shutdown();
    teardown.complete();

    let report = shutdown
        .wait_detailed()
        .await
        .expect_err("recorded failures must fail shutdown");
    assert_eq!(
        report,
        AgentTreeShutdownReport {
            tree_id: report.tree_id,
            failures: vec![failure; MAX_RETAINED_SHUTDOWN_FAILURES],
            omitted_failures: 3,
        }
    );
}
