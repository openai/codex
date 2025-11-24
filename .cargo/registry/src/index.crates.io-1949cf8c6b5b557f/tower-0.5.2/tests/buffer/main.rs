#![cfg(feature = "buffer")]
#[path = "../support.rs"]
mod support;
use std::thread;
use tokio_test::{assert_pending, assert_ready, assert_ready_err, assert_ready_ok, task};
use tower::buffer::{error, Buffer};
use tower::{util::ServiceExt, Service};
use tower_test::{assert_request_eq, mock};

fn let_worker_work() {
    // Allow the Buffer's executor to do work
    thread::sleep(::std::time::Duration::from_millis(100));
}

#[tokio::test(flavor = "current_thread")]
async fn req_and_res() {
    let _t = support::trace_init();

    let (mut service, mut handle) = new_service();

    assert_ready_ok!(service.poll_ready());
    let mut response = task::spawn(service.call("hello"));

    assert_request_eq!(handle, "hello").send_response("world");

    let_worker_work();
    assert_eq!(assert_ready_ok!(response.poll()), "world");
}

#[tokio::test(flavor = "current_thread")]
async fn clears_canceled_requests() {
    let _t = support::trace_init();

    let (mut service, mut handle) = new_service();

    handle.allow(1);

    assert_ready_ok!(service.poll_ready());
    let mut res1 = task::spawn(service.call("hello"));

    let send_response1 = assert_request_eq!(handle, "hello");

    // don't respond yet, new requests will get buffered
    assert_ready_ok!(service.poll_ready());
    let res2 = task::spawn(service.call("hello2"));

    assert_pending!(handle.poll_request());

    assert_ready_ok!(service.poll_ready());
    let mut res3 = task::spawn(service.call("hello3"));

    drop(res2);

    send_response1.send_response("world");

    let_worker_work();
    assert_eq!(assert_ready_ok!(res1.poll()), "world");

    // res2 was dropped, so it should have been canceled in the buffer
    handle.allow(1);

    assert_request_eq!(handle, "hello3").send_response("world3");

    let_worker_work();
    assert_eq!(assert_ready_ok!(res3.poll()), "world3");
}

#[tokio::test(flavor = "current_thread")]
async fn when_inner_is_not_ready() {
    let _t = support::trace_init();

    let (mut service, mut handle) = new_service();

    // Make the service NotReady
    handle.allow(0);

    assert_ready_ok!(service.poll_ready());
    let mut res1 = task::spawn(service.call("hello"));

    let_worker_work();
    assert_pending!(res1.poll());
    assert_pending!(handle.poll_request());

    handle.allow(1);

    assert_request_eq!(handle, "hello").send_response("world");

    let_worker_work();
    assert_eq!(assert_ready_ok!(res1.poll()), "world");
}

#[tokio::test(flavor = "current_thread")]
async fn when_inner_fails() {
    use std::error::Error as StdError;
    let _t = support::trace_init();

    let (mut service, mut handle) = new_service();

    // Make the service NotReady
    handle.allow(0);
    handle.send_error("foobar");

    assert_ready_ok!(service.poll_ready());
    let mut res1 = task::spawn(service.call("hello"));

    let_worker_work();
    let e = assert_ready_err!(res1.poll());
    if let Some(e) = e.downcast_ref::<error::ServiceError>() {
        let e = e.source().unwrap();

        assert_eq!(e.to_string(), "foobar");
    } else {
        panic!("unexpected error type: {:?}", e);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn poll_ready_when_worker_is_dropped_early() {
    let _t = support::trace_init();

    let (service, _handle) = mock::pair::<(), ()>();

    let (service, worker) = Buffer::pair(service, 1);

    let mut service = mock::Spawn::new(service);

    drop(worker);

    let err = assert_ready_err!(service.poll_ready());

    assert!(err.is::<error::Closed>(), "should be a Closed: {:?}", err);
}

#[tokio::test(flavor = "current_thread")]
async fn response_future_when_worker_is_dropped_early() {
    let _t = support::trace_init();

    let (service, mut handle) = mock::pair::<_, ()>();

    let (service, worker) = Buffer::pair(service, 1);

    let mut service = mock::Spawn::new(service);

    // keep the request in the worker
    handle.allow(0);
    assert_ready_ok!(service.poll_ready());
    let mut response = task::spawn(service.call("hello"));

    drop(worker);

    let_worker_work();
    let err = assert_ready_err!(response.poll());
    assert!(err.is::<error::Closed>(), "should be a Closed: {:?}", err);
}

#[tokio::test(flavor = "current_thread")]
async fn waits_for_channel_capacity() {
    let _t = support::trace_init();

    let (service, mut handle) = mock::pair::<&'static str, &'static str>();

    let (service, worker) = Buffer::pair(service, 2);

    let mut service = mock::Spawn::new(service);
    let mut worker = task::spawn(worker);

    // keep requests in the worker
    handle.allow(0);
    assert_ready_ok!(service.poll_ready());
    let mut response1 = task::spawn(service.call("hello"));
    assert_pending!(worker.poll());

    assert_ready_ok!(service.poll_ready());
    let mut response2 = task::spawn(service.call("hello"));
    assert_pending!(worker.poll());

    assert_ready_ok!(service.poll_ready());
    let mut response3 = task::spawn(service.call("hello"));
    assert_pending!(service.poll_ready());
    assert_pending!(worker.poll());

    handle.allow(1);
    assert_pending!(worker.poll());

    handle
        .next_request()
        .await
        .unwrap()
        .1
        .send_response("world");
    assert_pending!(worker.poll());
    assert_ready_ok!(response1.poll());

    assert_ready_ok!(service.poll_ready());
    let mut response4 = task::spawn(service.call("hello"));
    assert_pending!(worker.poll());

    handle.allow(3);
    assert_pending!(worker.poll());

    handle
        .next_request()
        .await
        .unwrap()
        .1
        .send_response("world");
    assert_pending!(worker.poll());
    assert_ready_ok!(response2.poll());

    assert_pending!(worker.poll());
    handle
        .next_request()
        .await
        .unwrap()
        .1
        .send_response("world");
    assert_pending!(worker.poll());
    assert_ready_ok!(response3.poll());

    assert_pending!(worker.poll());
    handle
        .next_request()
        .await
        .unwrap()
        .1
        .send_response("world");
    assert_pending!(worker.poll());
    assert_ready_ok!(response4.poll());
}

#[tokio::test(flavor = "current_thread")]
async fn wakes_pending_waiters_on_close() {
    let _t = support::trace_init();

    let (service, mut handle) = mock::pair::<_, ()>();

    let (mut service, worker) = Buffer::pair(service, 1);
    let mut worker = task::spawn(worker);

    // keep the request in the worker
    handle.allow(0);
    let service1 = service.ready().await.unwrap();
    assert_pending!(worker.poll());
    let mut response = task::spawn(service1.call("hello"));

    assert!(worker.is_woken(), "worker task should be woken by request");
    assert_pending!(worker.poll());

    // fill the channel so all subsequent requests will wait for capacity
    let service1 = assert_ready_ok!(task::spawn(service.ready()).poll());
    assert_pending!(worker.poll());
    let mut response2 = task::spawn(service1.call("world"));

    let mut service1 = service.clone();
    let mut ready1 = task::spawn(service1.ready());
    assert_pending!(worker.poll());
    assert_pending!(ready1.poll(), "no capacity");

    let mut service1 = service.clone();
    let mut ready2 = task::spawn(service1.ready());
    assert_pending!(worker.poll());
    assert_pending!(ready2.poll(), "no capacity");

    // kill the worker task
    drop(worker);

    let err = assert_ready_err!(response.poll());
    assert!(
        err.is::<error::Closed>(),
        "response should fail with a Closed, got: {:?}",
        err
    );

    let err = assert_ready_err!(response2.poll());
    assert!(
        err.is::<error::Closed>(),
        "response should fail with a Closed, got: {:?}",
        err
    );

    assert!(
        ready1.is_woken(),
        "dropping worker should wake ready task 1"
    );
    let err = assert_ready_err!(ready1.poll());
    assert!(
        err.is::<error::Closed>(),
        "ready 1 should fail with a Closed, got: {:?}",
        err
    );

    assert!(
        ready2.is_woken(),
        "dropping worker should wake ready task 2"
    );
    let err = assert_ready_err!(ready1.poll());
    assert!(
        err.is::<error::Closed>(),
        "ready 2 should fail with a Closed, got: {:?}",
        err
    );
}

#[tokio::test(flavor = "current_thread")]
async fn wakes_pending_waiters_on_failure() {
    let _t = support::trace_init();

    let (service, mut handle) = mock::pair::<_, ()>();

    let (mut service, worker) = Buffer::pair(service, 1);
    let mut worker = task::spawn(worker);

    // keep the request in the worker
    handle.allow(0);
    let service1 = service.ready().await.unwrap();
    assert_pending!(worker.poll());
    let mut response = task::spawn(service1.call("hello"));

    assert!(worker.is_woken(), "worker task should be woken by request");
    assert_pending!(worker.poll());

    // fill the channel so all subsequent requests will wait for capacity
    let service1 = assert_ready_ok!(task::spawn(service.ready()).poll());
    assert_pending!(worker.poll());
    let mut response2 = task::spawn(service1.call("world"));

    let mut service1 = service.clone();
    let mut ready1 = task::spawn(service1.ready());
    assert_pending!(worker.poll());
    assert_pending!(ready1.poll(), "no capacity");

    let mut service1 = service.clone();
    let mut ready2 = task::spawn(service1.ready());
    assert_pending!(worker.poll());
    assert_pending!(ready2.poll(), "no capacity");

    // fail the inner service
    handle.send_error("foobar");
    // worker task terminates
    assert_ready!(worker.poll());

    let err = assert_ready_err!(response.poll());
    assert!(
        err.is::<error::ServiceError>(),
        "response should fail with a ServiceError, got: {:?}",
        err
    );
    let err = assert_ready_err!(response2.poll());
    assert!(
        err.is::<error::ServiceError>(),
        "response should fail with a ServiceError, got: {:?}",
        err
    );

    assert!(
        ready1.is_woken(),
        "dropping worker should wake ready task 1"
    );
    let err = assert_ready_err!(ready1.poll());
    assert!(
        err.is::<error::ServiceError>(),
        "ready 1 should fail with a ServiceError, got: {:?}",
        err
    );

    assert!(
        ready2.is_woken(),
        "dropping worker should wake ready task 2"
    );
    let err = assert_ready_err!(ready1.poll());
    assert!(
        err.is::<error::ServiceError>(),
        "ready 2 should fail with a ServiceError, got: {:?}",
        err
    );
}

#[tokio::test(flavor = "current_thread")]
async fn propagates_trace_spans() {
    use tower::util::ServiceExt;
    use tracing::Instrument;

    let _t = support::trace_init();

    let span = tracing::info_span!("my_span");

    let service = support::AssertSpanSvc::new(span.clone());
    let (service, worker) = Buffer::pair(service, 5);
    let worker = tokio::spawn(worker);

    let result = tokio::spawn(service.oneshot(()).instrument(span));

    result.await.expect("service panicked").expect("failed");
    worker.await.expect("worker panicked");
}

#[tokio::test(flavor = "current_thread")]
async fn doesnt_leak_permits() {
    let _t = support::trace_init();

    let (service, mut handle) = mock::pair::<_, ()>();

    let (mut service1, worker) = Buffer::pair(service, 2);
    let mut worker = task::spawn(worker);
    let mut service2 = service1.clone();
    let mut service3 = service1.clone();

    // Attempt to poll the first clone of the buffer to readiness multiple
    // times. These should all succeed, because the readiness is never
    // *consumed* --- no request is sent.
    assert_ready_ok!(task::spawn(service1.ready()).poll());
    assert_ready_ok!(task::spawn(service1.ready()).poll());
    assert_ready_ok!(task::spawn(service1.ready()).poll());

    // It should also be possible to drive the second clone of the service to
    // readiness --- it should only acquire one permit, as well.
    assert_ready_ok!(task::spawn(service2.ready()).poll());
    assert_ready_ok!(task::spawn(service2.ready()).poll());
    assert_ready_ok!(task::spawn(service2.ready()).poll());

    // The third clone *doesn't* poll ready, because the first two clones have
    // each acquired one permit.
    let mut ready3 = task::spawn(service3.ready());
    assert_pending!(ready3.poll());

    // Consume the first service's readiness.
    let mut response = task::spawn(service1.call(()));
    handle.allow(1);
    assert_pending!(worker.poll());

    handle.next_request().await.unwrap().1.send_response(());
    assert_pending!(worker.poll());
    assert_ready_ok!(response.poll());

    // Now, the third service should acquire a permit...
    assert!(ready3.is_woken());
    assert_ready_ok!(ready3.poll());
}

type Handle = mock::Handle<&'static str, &'static str>;
type MockBuffer = Buffer<&'static str, mock::future::ResponseFuture<&'static str>>;

fn new_service() -> (mock::Spawn<MockBuffer>, Handle) {
    // bound is >0 here because clears_canceled_requests needs multiple outstanding requests
    new_service_with_bound(10)
}

fn new_service_with_bound(bound: usize) -> (mock::Spawn<MockBuffer>, Handle) {
    mock::spawn_with(|s| {
        let (svc, worker) = Buffer::pair(s, bound);

        thread::spawn(move || {
            let mut fut = tokio_test::task::spawn(worker);
            while fut.poll().is_pending() {}
        });

        svc
    })
}
