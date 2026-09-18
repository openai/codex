//! Checks shared execution-guard ownership through completion and cancellation.

use super::AgentExecutionGuard;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::sync::oneshot;

#[tokio::test]
async fn dropping_guard_releases_backend_reservation() {
    let capacity = Arc::new(Semaphore::new(/*permits*/ 1));
    let permit = Arc::clone(&capacity).acquire_owned().await.unwrap();
    let guard = AgentExecutionGuard::new(permit);
    assert_eq!(capacity.available_permits(), 0);
    drop(guard);
    assert_eq!(capacity.available_permits(), 1);
}

#[tokio::test]
async fn cancelling_turn_releases_backend_reservation() {
    let capacity = Arc::new(Semaphore::new(/*permits*/ 1));
    let permit = Arc::clone(&capacity).acquire_owned().await.unwrap();
    let (started, running) = oneshot::channel();
    let turn = tokio::spawn(async move {
        let _guard = AgentExecutionGuard::new(permit);
        started.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    running.await.unwrap();
    assert_eq!(capacity.available_permits(), 0);
    turn.abort();
    assert!(turn.await.unwrap_err().is_cancelled());
    assert_eq!(capacity.available_permits(), 1);
}
