use std::sync::Arc;
use std::time::Duration;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::StartedCell;
use codex_code_mode_protocol::host::DelegateRequest;
use codex_code_mode_protocol::host::RequestId;
use codex_code_mode_protocol::host::SessionId;
use pretty_assertions::assert_eq;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::TryRecvError;
use tokio_util::sync::CancellationToken;

use super::HostPeer;
use super::MAX_PENDING_DELEGATE_CALLS;

fn session_id(value: &str) -> SessionId {
    SessionId::new(value).expect("session ID")
}

#[tokio::test]
async fn start_cell_reports_when_initial_response_is_enqueued() {
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel(/*max_capacity*/ 4);
    let peer = Arc::new(HostPeer::new(outgoing_tx));
    let cell_id = CellId::new("cell-1".to_string());
    let (response_tx, response_rx) = oneshot::channel();
    let started = StartedCell::new(cell_id.clone(), response_rx);
    let active_cell_permits = Arc::new(Semaphore::new(/*permits*/ 1));
    let active_cell_permit = Arc::clone(&active_cell_permits)
        .try_acquire_owned()
        .expect("active cell permit");

    let mut initial_response_sent = peer.start_cell(
        session_id("session-1"),
        RequestId::new(/*value*/ 1),
        started,
        active_cell_permit,
    );
    assert_eq!(initial_response_sent.try_recv(), Err(TryRecvError::Empty));

    response_tx
        .send(RuntimeResponse::Result {
            cell_id: cell_id.clone(),
            content_items: Vec::new(),
            error_text: None,
        })
        .expect("initial response receiver");
    initial_response_sent
        .await
        .expect("initial response completion");
    outgoing_rx.recv().await.expect("initial response frame");
    assert_eq!(active_cell_permits.available_permits(), 0);

    peer.close_cell(session_id("session-1"), cell_id);
    let permit = tokio::time::timeout(
        Duration::from_secs(1),
        Arc::clone(&active_cell_permits).acquire_owned(),
    )
    .await
    .expect("cell permit should be released")
    .expect("cell permit semaphore should remain open");
    drop(permit);
}

#[tokio::test]
async fn pending_delegate_limit_rejects_call_without_disconnecting() {
    let (outgoing_tx, _outgoing_rx) = mpsc::channel(/*max_capacity*/ 1);
    let peer = Arc::new(HostPeer::new(outgoing_tx));
    let permits = Arc::clone(&peer.delegate_permits)
        .acquire_many_owned(MAX_PENDING_DELEGATE_CALLS as u32)
        .await
        .expect("delegate permits");

    let result = peer
        .call(
            session_id("session-1"),
            DelegateRequest::Notify {
                call_id: "call-1".to_string(),
                cell_id: CellId::new("cell-1".to_string()).into(),
                text: "hello".to_string(),
            },
            CancellationToken::new(),
        )
        .await;

    assert_eq!(
        result,
        Err("code-mode host has too many pending delegate calls".to_string())
    );
    assert!(!peer.is_disconnected());
    drop(permits);
}
