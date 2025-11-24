#![cfg(feature = "load-shed")]
#[path = "../support.rs"]
mod support;

use tokio_test::{assert_ready_err, assert_ready_ok, task};
use tower::load_shed::LoadShedLayer;
use tower_test::{assert_request_eq, mock};

#[tokio::test(flavor = "current_thread")]
async fn when_ready() {
    let _t = support::trace_init();

    let layer = LoadShedLayer::new();
    let (mut service, mut handle) = mock::spawn_layer(layer);

    assert_ready_ok!(service.poll_ready(), "overload always reports ready");

    let mut response = task::spawn(service.call("hello"));

    assert_request_eq!(handle, "hello").send_response("world");
    assert_eq!(assert_ready_ok!(response.poll()), "world");
}

#[tokio::test(flavor = "current_thread")]
async fn when_not_ready() {
    let _t = support::trace_init();

    let layer = LoadShedLayer::new();
    let (mut service, mut handle) = mock::spawn_layer::<_, (), _>(layer);

    handle.allow(0);

    assert_ready_ok!(service.poll_ready(), "overload always reports ready");

    let mut fut = task::spawn(service.call("hello"));

    let err = assert_ready_err!(fut.poll());
    assert!(err.is::<tower::load_shed::error::Overloaded>());
}
