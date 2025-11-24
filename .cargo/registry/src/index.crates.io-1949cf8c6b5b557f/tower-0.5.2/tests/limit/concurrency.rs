#[path = "../support.rs"]
mod support;
use tokio_test::{assert_pending, assert_ready, assert_ready_ok};
use tower::limit::concurrency::ConcurrencyLimitLayer;
use tower_test::{assert_request_eq, mock};

#[tokio::test(flavor = "current_thread")]
async fn basic_service_limit_functionality_with_poll_ready() {
    let _t = support::trace_init();
    let limit = ConcurrencyLimitLayer::new(2);
    let (mut service, mut handle) = mock::spawn_layer(limit);

    assert_ready_ok!(service.poll_ready());
    let r1 = service.call("hello 1");

    assert_ready_ok!(service.poll_ready());
    let r2 = service.call("hello 2");

    assert_pending!(service.poll_ready());

    assert!(!service.is_woken());

    // The request gets passed through
    assert_request_eq!(handle, "hello 1").send_response("world 1");

    // The next request gets passed through
    assert_request_eq!(handle, "hello 2").send_response("world 2");

    // There are no more requests
    assert_pending!(handle.poll_request());

    assert_eq!(r1.await.unwrap(), "world 1");

    assert!(service.is_woken());

    // Another request can be sent
    assert_ready_ok!(service.poll_ready());

    let r3 = service.call("hello 3");

    assert_pending!(service.poll_ready());

    assert_eq!(r2.await.unwrap(), "world 2");

    // The request gets passed through
    assert_request_eq!(handle, "hello 3").send_response("world 3");

    assert_eq!(r3.await.unwrap(), "world 3");
}

#[tokio::test(flavor = "current_thread")]
async fn basic_service_limit_functionality_without_poll_ready() {
    let _t = support::trace_init();
    let limit = ConcurrencyLimitLayer::new(2);
    let (mut service, mut handle) = mock::spawn_layer(limit);

    assert_ready_ok!(service.poll_ready());
    let r1 = service.call("hello 1");

    assert_ready_ok!(service.poll_ready());
    let r2 = service.call("hello 2");

    assert_pending!(service.poll_ready());

    // The request gets passed through
    assert_request_eq!(handle, "hello 1").send_response("world 1");

    assert!(!service.is_woken());

    // The next request gets passed through
    assert_request_eq!(handle, "hello 2").send_response("world 2");

    assert!(!service.is_woken());

    // There are no more requests
    assert_pending!(handle.poll_request());

    assert_eq!(r1.await.unwrap(), "world 1");

    assert!(service.is_woken());

    // One more request can be sent
    assert_ready_ok!(service.poll_ready());
    let r4 = service.call("hello 4");

    assert_pending!(service.poll_ready());

    assert_eq!(r2.await.unwrap(), "world 2");
    assert!(service.is_woken());

    // The request gets passed through
    assert_request_eq!(handle, "hello 4").send_response("world 4");

    assert_eq!(r4.await.unwrap(), "world 4");
}

#[tokio::test(flavor = "current_thread")]
async fn request_without_capacity() {
    let _t = support::trace_init();
    let limit = ConcurrencyLimitLayer::new(0);
    let (mut service, _) = mock::spawn_layer::<(), (), _>(limit);

    assert_pending!(service.poll_ready());
}

#[tokio::test(flavor = "current_thread")]
async fn reserve_capacity_without_sending_request() {
    let _t = support::trace_init();
    let limit = ConcurrencyLimitLayer::new(1);
    let (mut s1, mut handle) = mock::spawn_layer(limit);

    let mut s2 = s1.clone();

    // Reserve capacity in s1
    assert_ready_ok!(s1.poll_ready());

    // Service 2 cannot get capacity
    assert_pending!(s2.poll_ready());

    // s1 sends the request, then s2 is able to get capacity
    let r1 = s1.call("hello");

    assert_request_eq!(handle, "hello").send_response("world");

    assert_pending!(s2.poll_ready());

    r1.await.unwrap();

    assert_ready_ok!(s2.poll_ready());
}

#[tokio::test(flavor = "current_thread")]
async fn service_drop_frees_capacity() {
    let _t = support::trace_init();
    let limit = ConcurrencyLimitLayer::new(1);
    let (mut s1, _handle) = mock::spawn_layer::<(), (), _>(limit);

    let mut s2 = s1.clone();

    // Reserve capacity in s1
    assert_ready_ok!(s1.poll_ready());

    // Service 2 cannot get capacity
    assert_pending!(s2.poll_ready());

    drop(s1);

    assert!(s2.is_woken());
    assert_ready_ok!(s2.poll_ready());
}

#[tokio::test(flavor = "current_thread")]
async fn response_error_releases_capacity() {
    let _t = support::trace_init();
    let limit = ConcurrencyLimitLayer::new(1);
    let (mut s1, mut handle) = mock::spawn_layer::<_, (), _>(limit);

    let mut s2 = s1.clone();

    // Reserve capacity in s1
    assert_ready_ok!(s1.poll_ready());

    // s1 sends the request, then s2 is able to get capacity
    let r1 = s1.call("hello");

    assert_request_eq!(handle, "hello").send_error("boom");

    r1.await.unwrap_err();

    assert_ready_ok!(s2.poll_ready());
}

#[tokio::test(flavor = "current_thread")]
async fn response_future_drop_releases_capacity() {
    let _t = support::trace_init();
    let limit = ConcurrencyLimitLayer::new(1);
    let (mut s1, _handle) = mock::spawn_layer::<_, (), _>(limit);

    let mut s2 = s1.clone();

    // Reserve capacity in s1
    assert_ready_ok!(s1.poll_ready());

    // s1 sends the request, then s2 is able to get capacity
    let r1 = s1.call("hello");

    assert_pending!(s2.poll_ready());

    drop(r1);

    assert_ready_ok!(s2.poll_ready());
}

#[tokio::test(flavor = "current_thread")]
async fn multi_waiters() {
    let _t = support::trace_init();
    let limit = ConcurrencyLimitLayer::new(1);
    let (mut s1, _handle) = mock::spawn_layer::<(), (), _>(limit);
    let mut s2 = s1.clone();
    let mut s3 = s1.clone();

    // Reserve capacity in s1
    assert_ready_ok!(s1.poll_ready());

    // s2 and s3 are not ready
    assert_pending!(s2.poll_ready());
    assert_pending!(s3.poll_ready());

    drop(s1);

    assert!(s2.is_woken());
    assert!(!s3.is_woken());

    drop(s2);

    assert!(s3.is_woken());
}
