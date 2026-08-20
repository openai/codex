use tokio::sync::oneshot;

use super::AcceptedConnectionSource;
use crate::ExecServerClientConnectOptions;

#[tokio::test]
async fn replacement_claim_is_released_when_handoff_is_cancelled() {
    let source = AcceptedConnectionSource::new(ExecServerClientConnectOptions::default());
    let (claimed_tx, claimed_rx) = oneshot::channel();
    let (_release_tx, release_rx) = oneshot::channel::<()>();

    let handoff = tokio::spawn({
        let source = source.clone();
        async move {
            let _submission = source
                .begin_replacement()
                .expect("the first replacement should claim the handoff");
            claimed_tx
                .send(())
                .expect("the test should wait for the claim");
            let _ = release_rx.await;
        }
    });

    claimed_rx.await.expect("the handoff should be claimed");
    assert!(source.begin_replacement().is_err());

    handoff.abort();
    let _ = handoff.await;

    source
        .begin_replacement()
        .expect("a new replacement should be accepted after cancellation");
}
