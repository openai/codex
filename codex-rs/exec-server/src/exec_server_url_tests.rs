use pretty_assertions::assert_eq;

use super::ExecServerUrl;

#[tokio::test]
async fn ready_url_resolves_immediately() {
    let url = ExecServerUrl::ready("ws://127.0.0.1:1234".to_string());

    assert_eq!(
        url.resolve().await.expect("ready URL"),
        "ws://127.0.0.1:1234"
    );
    assert_eq!(url.current(), Some("ws://127.0.0.1:1234"));
}

#[tokio::test]
async fn pending_url_resolves_once_and_caches_the_result() {
    let url = ExecServerUrl::pending();
    let waiter = tokio::spawn({
        let url = url.clone();
        async move { url.resolve().await }
    });
    tokio::task::yield_now().await;
    assert!(!waiter.is_finished());

    url.set_ready("ws://127.0.0.1:5678".to_string())
        .expect("complete URL");

    assert_eq!(
        waiter.await.expect("waiter task").expect("resolved URL"),
        "ws://127.0.0.1:5678"
    );
    assert_eq!(
        url.resolve().await.expect("cached URL"),
        "ws://127.0.0.1:5678"
    );
    assert_eq!(url.current(), Some("ws://127.0.0.1:5678"));
    assert_eq!(
        url.set_ready("ws://127.0.0.1:9999".to_string())
            .expect_err("URL completion is one-shot")
            .to_string(),
        "exec-server protocol error: exec-server URL is not pending"
    );
}

#[tokio::test]
async fn pending_url_failure_is_shared() {
    let url = ExecServerUrl::pending();

    url.set_failed("provisioning failed".to_string())
        .expect("fail URL");

    assert_eq!(
        url.resolve().await.expect_err("failed URL").to_string(),
        "environment unavailable: provisioning failed"
    );
    assert_eq!(
        url.resolve()
            .await
            .expect_err("cached failed URL")
            .to_string(),
        "environment unavailable: provisioning failed"
    );
    assert_eq!(url.current(), None);
}
