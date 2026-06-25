use std::time::Duration;

use codex_code_mode_protocol::RuntimeResponse;
use pretty_assertions::assert_eq;

use super::*;

#[tokio::test]
async fn registration_queues_cell_closure_before_driver_starts() {
    let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel();
    let peer = Arc::new(HostPeer::new(outgoing_tx));
    let session_id = SessionId::new("session-1").expect("session ID");
    let cell_id = CellId::new("cell-1".to_string());
    let request_id = RequestId::new(/*value*/ 7);
    let registration = peer
        .register_cell(session_id.clone(), cell_id.clone())
        .expect("register cell");

    peer.close_cell(session_id.clone(), cell_id.clone());
    assert!(outgoing_rx.try_recv().is_err());

    let (initial_tx, initial_rx) = oneshot::channel();
    peer.start_cell(
        registration,
        request_id,
        StartedCell::from_result_receiver(cell_id.clone(), initial_rx),
    );
    let response = RuntimeResponse::Result {
        cell_id: cell_id.clone(),
        content_items: Vec::new(),
        error_text: None,
    };
    initial_tx
        .send(Ok(response.clone()))
        .expect("send initial response");

    assert_eq!(
        outgoing_rx.recv().await,
        Some(HostToClient::InitialResponse {
            id: request_id,
            result: WireResult::Ok {
                value: response.into(),
            },
        })
    );
    assert_eq!(
        outgoing_rx.recv().await,
        Some(HostToClient::CellClosed {
            session_id,
            cell_id: cell_id.into(),
        })
    );
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if peer
                .cell_routes
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .is_empty()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cell route cleanup");
}
