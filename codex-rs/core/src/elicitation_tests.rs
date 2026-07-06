use super::*;

#[tokio::test]
async fn wait_until_clear_waits_for_every_registration() {
    let service = ElicitationService::new();
    let first = service.register();
    let second = service.register();
    let waiting = tokio::spawn({
        let service = service.clone();
        async move { service.wait_until_clear().await }
    });

    drop(first);
    tokio::task::yield_now().await;
    assert!(!waiting.is_finished());

    drop(second);
    waiting.await.expect("elicitation waiter should complete");
}
