#![cfg(feature = "hedge")]
#[path = "../support.rs"]
mod support;

use std::time::Duration;
use tokio::time;
use tokio_test::{assert_pending, assert_ready, assert_ready_ok, task};
use tower::hedge::{Hedge, Policy};
use tower_test::{assert_request_eq, mock};

#[tokio::test(flavor = "current_thread")]
async fn hedge_orig_completes_first() {
    let _t = support::trace_init();
    time::pause();

    let (mut service, mut handle) = new_service(TestPolicy);

    assert_ready_ok!(service.poll_ready());
    let mut fut = task::spawn(service.call("orig"));

    // Check that orig request has been issued.
    let req = assert_request_eq!(handle, "orig");
    // Check fut is not ready.
    assert_pending!(fut.poll());

    // Check hedge has not been issued.
    assert_pending!(handle.poll_request());
    time::advance(Duration::from_millis(11)).await;
    // Check fut is not ready.
    assert_pending!(fut.poll());
    // Check that the hedge has been issued.
    let _hedge_req = assert_request_eq!(handle, "orig");

    req.send_response("orig-done");
    // Check that fut gets orig response.
    assert_eq!(assert_ready_ok!(fut.poll()), "orig-done");
}

#[tokio::test(flavor = "current_thread")]
async fn hedge_hedge_completes_first() {
    let _t = support::trace_init();
    time::pause();

    let (mut service, mut handle) = new_service(TestPolicy);

    assert_ready_ok!(service.poll_ready());
    let mut fut = task::spawn(service.call("orig"));

    // Check that orig request has been issued.
    let _req = assert_request_eq!(handle, "orig");

    // Check fut is not ready.
    assert_pending!(fut.poll());

    // Check hedge has not been issued.
    assert_pending!(handle.poll_request());
    time::advance(Duration::from_millis(11)).await;
    // Check fut is not ready.
    assert_pending!(fut.poll());

    // Check that the hedge has been issued.
    let hedge_req = assert_request_eq!(handle, "orig");
    hedge_req.send_response("hedge-done");
    // Check that fut gets hedge response.
    assert_eq!(assert_ready_ok!(fut.poll()), "hedge-done");
}

#[tokio::test(flavor = "current_thread")]
async fn completes_before_hedge() {
    let _t = support::trace_init();
    let (mut service, mut handle) = new_service(TestPolicy);

    assert_ready_ok!(service.poll_ready());
    let mut fut = task::spawn(service.call("orig"));

    // Check that orig request has been issued.
    let req = assert_request_eq!(handle, "orig");
    // Check fut is not ready.
    assert_pending!(fut.poll());

    req.send_response("orig-done");
    // Check hedge has not been issued.
    assert_pending!(handle.poll_request());
    // Check that fut gets orig response.
    assert_eq!(assert_ready_ok!(fut.poll()), "orig-done");
}

#[tokio::test(flavor = "current_thread")]
async fn request_not_retyable() {
    let _t = support::trace_init();
    time::pause();

    let (mut service, mut handle) = new_service(TestPolicy);

    assert_ready_ok!(service.poll_ready());
    let mut fut = task::spawn(service.call(NOT_RETRYABLE));

    // Check that orig request has been issued.
    let req = assert_request_eq!(handle, NOT_RETRYABLE);
    // Check fut is not ready.
    assert_pending!(fut.poll());

    // Check hedge has not been issued.
    assert_pending!(handle.poll_request());
    time::advance(Duration::from_millis(10)).await;
    // Check fut is not ready.
    assert_pending!(fut.poll());
    // Check hedge has not been issued.
    assert_pending!(handle.poll_request());

    req.send_response("orig-done");
    // Check that fut gets orig response.
    assert_eq!(assert_ready_ok!(fut.poll()), "orig-done");
}

#[tokio::test(flavor = "current_thread")]
async fn request_not_clonable() {
    let _t = support::trace_init();
    time::pause();

    let (mut service, mut handle) = new_service(TestPolicy);

    assert_ready_ok!(service.poll_ready());
    let mut fut = task::spawn(service.call(NOT_CLONABLE));

    // Check that orig request has been issued.
    let req = assert_request_eq!(handle, NOT_CLONABLE);
    // Check fut is not ready.
    assert_pending!(fut.poll());

    // Check hedge has not been issued.
    assert_pending!(handle.poll_request());
    time::advance(Duration::from_millis(10)).await;
    // Check fut is not ready.
    assert_pending!(fut.poll());
    // Check hedge has not been issued.
    assert_pending!(handle.poll_request());

    req.send_response("orig-done");
    // Check that fut gets orig response.
    assert_eq!(assert_ready_ok!(fut.poll()), "orig-done");
}

type Req = &'static str;
type Res = &'static str;
type Mock = tower_test::mock::Mock<Req, Res>;
type Handle = tower_test::mock::Handle<Req, Res>;

static NOT_RETRYABLE: &str = "NOT_RETRYABLE";
static NOT_CLONABLE: &str = "NOT_CLONABLE";

#[derive(Clone)]
struct TestPolicy;

impl tower::hedge::Policy<Req> for TestPolicy {
    fn can_retry(&self, req: &Req) -> bool {
        *req != NOT_RETRYABLE
    }

    fn clone_request(&self, req: &Req) -> Option<Req> {
        if *req == NOT_CLONABLE {
            None
        } else {
            Some(req)
        }
    }
}

fn new_service<P: Policy<Req> + Clone>(policy: P) -> (mock::Spawn<Hedge<Mock, P>>, Handle) {
    let (service, handle) = tower_test::mock::pair();

    let mock_latencies: [u64; 10] = [1, 1, 1, 1, 1, 1, 1, 1, 10, 10];

    let service = Hedge::new_with_mock_latencies(
        service,
        policy,
        10,
        0.9,
        Duration::from_secs(60),
        &mock_latencies,
    );

    (mock::Spawn::new(service), handle)
}
