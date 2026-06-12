use pretty_assertions::assert_eq;
use tokio::sync::oneshot;

use super::CellId;
use super::StartedCell;

#[tokio::test]
async fn started_cell_preserves_remote_initial_response_errors() {
    let (response_tx, response_rx) = oneshot::channel();
    response_tx
        .send(Err("remote runtime failed".to_string()))
        .expect("initial response receiver should be open");
    let started = StartedCell::from_result_receiver(CellId::new("1".to_string()), response_rx);

    assert_eq!(
        started.initial_response().await,
        Err("remote runtime failed".to_string())
    );
}
