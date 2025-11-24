use super::support;
use std::time::Duration;
use tokio::time;
use tokio_test::{assert_pending, assert_ready, assert_ready_ok};
use tower::limit::rate::RateLimitLayer;
use tower_test::{assert_request_eq, mock};

#[tokio::test(flavor = "current_thread")]
async fn reaching_capacity() {
    let _t = support::trace_init();
    time::pause();

    let rate_limit = RateLimitLayer::new(1, Duration::from_millis(100));
    let (mut service, mut handle) = mock::spawn_layer(rate_limit);

    assert_ready_ok!(service.poll_ready());

    let response = service.call("hello");

    assert_request_eq!(handle, "hello").send_response("world");

    assert_eq!(response.await.unwrap(), "world");
    assert_pending!(service.poll_ready());

    assert_pending!(handle.poll_request());

    time::advance(Duration::from_millis(101)).await;

    assert_ready_ok!(service.poll_ready());

    let response = service.call("two");

    assert_request_eq!(handle, "two").send_response("done");

    assert_eq!(response.await.unwrap(), "done");
}

#[tokio::test(flavor = "current_thread")]
async fn remaining_gets_reset() {
    // This test checks for the case where the `until` state gets reset
    // but the `rem` does not. This was a bug found `cd7dd12315706fc0860a35646b1eb7b60c50a5c1`.
    //
    // The main premise here is that we can make one request which should initialize the state
    // as ready. Then we can advance the clock to put us beyond the current period. When we make
    // subsequent requests the `rem` for the next window is continued from the previous when
    // it should be totally reset.
    let _t = support::trace_init();
    time::pause();

    let rate_limit = RateLimitLayer::new(3, Duration::from_millis(100));
    let (mut service, mut handle) = mock::spawn_layer(rate_limit);

    assert_ready_ok!(service.poll_ready());
    let response = service.call("hello");
    assert_request_eq!(handle, "hello").send_response("world");
    assert_eq!(response.await.unwrap(), "world");

    time::advance(Duration::from_millis(100)).await;

    assert_ready_ok!(service.poll_ready());
    let response = service.call("hello");
    assert_request_eq!(handle, "hello").send_response("world");
    assert_eq!(response.await.unwrap(), "world");

    assert_ready_ok!(service.poll_ready());
    let response = service.call("hello");
    assert_request_eq!(handle, "hello").send_response("world");
    assert_eq!(response.await.unwrap(), "world");

    assert_ready_ok!(service.poll_ready());
}
