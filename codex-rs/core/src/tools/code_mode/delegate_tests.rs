use std::sync::Arc;
use std::time::Duration;

use codex_code_mode::CellId;
use codex_code_mode::CodeModeSessionDelegate;
use pretty_assertions::assert_eq;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::super::ExecContext;
use super::CodeModeDispatchBroker;
use crate::session::tests::make_session_and_context;
use crate::state::ActiveTurn;
use crate::tools::registry::ToolRegistry;
use crate::tools::router::ToolRouter;
use crate::turn_diff_tracker::TurnDiffTracker;

#[tokio::test]
#[expect(
    clippy::await_holding_invalid_type,
    reason = "test holds turn state to keep notification delivery blocked until cancellation"
)]
async fn cancelled_notification_does_not_keep_the_turn_worker_alive() {
    let (session, turn) = make_session_and_context().await;
    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let session_weak = Arc::downgrade(&session);
    let active_turn = ActiveTurn::default();
    let turn_state = Arc::clone(&active_turn.turn_state);
    *session.active_turn.lock().await = Some(active_turn);
    let _turn_state_guard = turn_state.lock().await;

    let broker = CodeModeDispatchBroker::new();
    let cell_id = CellId::new("cell-1".to_string());
    let cancellation_token = CancellationToken::new();
    let notification = broker.notify(
        "call-1".to_string(),
        cell_id,
        "pending".to_string(),
        cancellation_token.clone(),
    );
    tokio::pin!(notification);
    std::future::poll_fn(|context| match notification.as_mut().poll(context) {
        std::task::Poll::Pending => std::task::Poll::Ready(()),
        std::task::Poll::Ready(result) => {
            panic!("notification returned before cancellation: {result:?}")
        }
    })
    .await;
    assert_eq!(broker.dispatch_rx.len(), 1);

    let worker = broker.start_turn_worker(
        ExecContext {
            session: Arc::clone(&session),
            turn: Arc::clone(&turn),
        },
        Arc::new(ToolRouter::from_parts(
            ToolRegistry::empty_for_test(),
            Vec::new(),
        )),
        Arc::new(Mutex::new(TurnDiffTracker::new())),
    );
    tokio::time::timeout(Duration::from_millis(/*millis*/ 100), async {
        while !broker.dispatch_rx.is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("turn worker did not start the notification");

    cancellation_token.cancel();
    assert_eq!(
        notification.await,
        Err("code mode notification cancelled".to_string())
    );
    drop(worker);
    drop(turn);
    drop(session);

    tokio::time::timeout(Duration::from_millis(/*millis*/ 100), async {
        while session_weak.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancelled notification kept the turn worker alive");
}
