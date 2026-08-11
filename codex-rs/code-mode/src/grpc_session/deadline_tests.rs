use std::future::pending;
use std::sync::Arc;
use std::time::Duration;

use codex_code_mode_protocol::DEFAULT_EXEC_YIELD_TIME_MS;
use pretty_assertions::assert_eq;

use super::MAX_ERROR_BYTES;
use super::RequestError;
use super::enforce;
use super::startup;

#[tokio::test(start_paused = true)]
async fn stalled_transport_fails_after_its_deadline() {
    let task = tokio::spawn(enforce(
        "termination",
        Duration::ZERO,
        pending::<Result<(), tonic::Status>>(),
    ));
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(61)).await;

    assert!(matches!(
        task.await.expect("deadline task"),
        Err(RequestError::TimedOut(message))
            if message == "gRPC code-mode host timed out waiting for termination response"
    ));
}

#[tokio::test(start_paused = true)]
async fn requested_runtime_duration_is_added_to_the_transport_deadline() {
    let task = tokio::spawn(enforce(
        "wait",
        Duration::from_secs(120),
        pending::<Result<(), tonic::Status>>(),
    ));
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(61)).await;
    tokio::task::yield_now().await;
    assert!(!task.is_finished());

    tokio::time::advance(Duration::from_secs(120)).await;
    assert!(matches!(
        task.await.expect("deadline task"),
        Err(RequestError::TimedOut(_))
    ));
}

#[tokio::test(start_paused = true)]
async fn default_execution_yield_and_grace_extend_the_outcome_deadline() {
    let runtime_timeout =
        Duration::from_millis(DEFAULT_EXEC_YIELD_TIME_MS).saturating_add(Duration::from_secs(1));
    let task = tokio::spawn(enforce(
        "execution outcome",
        runtime_timeout,
        pending::<Result<(), tonic::Status>>(),
    ));
    tokio::task::yield_now().await;

    tokio::time::advance(Duration::from_secs(70)).await;
    tokio::task::yield_now().await;
    assert!(!task.is_finished());

    tokio::time::advance(Duration::from_secs(2)).await;
    assert!(matches!(
        task.await.expect("execution outcome deadline task"),
        Err(RequestError::TimedOut(message))
            if message == "gRPC code-mode host timed out waiting for execution outcome response"
    ));
}

#[tokio::test]
async fn transport_status_is_preserved() {
    let result = enforce("wait", Duration::ZERO, async {
        Err::<(), _>(tonic::Status::not_found("missing"))
    })
    .await;

    match result {
        Err(RequestError::Failed(error)) => {
            assert_eq!(error.code(), tonic::Code::NotFound);
            assert_eq!(error.message(), "missing");
        }
        _ => panic!("expected the original gRPC status"),
    }
}

#[tokio::test]
async fn transport_status_messages_are_bounded_at_utf8_boundaries() {
    let error = startup("session opening", async {
        Err::<(), _>(tonic::Status::internal("🦀".repeat(MAX_ERROR_BYTES)))
    })
    .await
    .expect_err("oversized gRPC status must fail");

    assert!(error.len() <= MAX_ERROR_BYTES);
    assert!(error.starts_with("gRPC code-mode session opening failed:"));
    assert!(error.ends_with("..."));
}

#[tokio::test(start_paused = true)]
async fn stalled_channel_acquisition_times_out_and_remains_retryable() {
    let channel = Arc::new(tokio::sync::OnceCell::new());
    let stalled_channel = Arc::clone(&channel);
    let stalled = tokio::spawn(async move {
        startup("transport connection", async {
            stalled_channel
                .get_or_try_init(pending::<Result<usize, String>>)
                .await
                .copied()
        })
        .await
    });
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(61)).await;

    assert_eq!(
        stalled.await.expect("channel connection task"),
        Err("gRPC code-mode host timed out waiting for transport connection response".to_string())
    );
    assert_eq!(
        startup("transport connection", async {
            channel
                .get_or_try_init(|| async { Ok::<_, String>(42usize) })
                .await
                .copied()
        })
        .await,
        Ok(42)
    );
}
