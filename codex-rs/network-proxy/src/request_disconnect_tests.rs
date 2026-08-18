use super::NetworkRequestDisconnect;
use pretty_assertions::assert_eq;
use std::future::pending;
use std::time::Duration;
use std::time::Instant;

#[tokio::test]
async fn disconnect_is_published_before_policy_cleanup() {
    struct ObserveOnDrop(NetworkRequestDisconnect);
    impl Drop for ObserveOnDrop {
        fn drop(&mut self) {
            assert!(self.0.elapsed().is_some());
        }
    }

    let disconnect = NetworkRequestDisconnect::default();
    let observer = ObserveOnDrop(disconnect.clone());
    let started_at = Instant::now();
    let decision = disconnect.track_http_request(started_at, async move {
        let _observer = observer;
        pending::<()>().await;
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(1), decision)
            .await
            .is_err()
    );
    assert!(disconnect.elapsed().expect("disconnect time") <= started_at.elapsed());
}

#[tokio::test]
async fn completed_policy_request_is_not_a_disconnect() {
    let disconnect = NetworkRequestDisconnect::default();
    assert_eq!(
        disconnect
            .track_http_request(Instant::now(), async { 42 })
            .await,
        42
    );
    assert_eq!(disconnect.elapsed(), None);
}
