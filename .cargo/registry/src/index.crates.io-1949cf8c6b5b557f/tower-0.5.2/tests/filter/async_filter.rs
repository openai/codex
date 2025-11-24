#![cfg(feature = "filter")]
#[path = "../support.rs"]
mod support;
use futures_util::{future::poll_fn, pin_mut};
use std::future::Future;
use tower::filter::{error::Error, AsyncFilter};
use tower_service::Service;
use tower_test::{assert_request_eq, mock};

#[tokio::test(flavor = "current_thread")]
async fn passthrough_sync() {
    let _t = support::trace_init();

    let (mut service, handle) = new_service(|_| async { Ok(()) });

    let th = tokio::spawn(async move {
        // Receive the requests and respond
        pin_mut!(handle);
        for i in 0..10usize {
            assert_request_eq!(handle, format!("ping-{}", i)).send_response(format!("pong-{}", i));
        }
    });

    let mut responses = vec![];

    for i in 0usize..10 {
        let request = format!("ping-{}", i);
        poll_fn(|cx| service.poll_ready(cx)).await.unwrap();
        let exchange = service.call(request);
        let exchange = async move {
            let response = exchange.await.unwrap();
            let expect = format!("pong-{}", i);
            assert_eq!(response.as_str(), expect.as_str());
        };

        responses.push(exchange);
    }

    futures_util::future::join_all(responses).await;
    th.await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn rejected_sync() {
    let _t = support::trace_init();

    let (mut service, _handle) = new_service(|_| async { Err(Error::rejected()) });

    service.call("hello".into()).await.unwrap_err();
}

type Mock = mock::Mock<String, String>;
type Handle = mock::Handle<String, String>;

fn new_service<F, U>(f: F) -> (AsyncFilter<Mock, F>, Handle)
where
    F: Fn(&String) -> U,
    U: Future<Output = Result<(), Error>>,
{
    let (service, handle) = mock::pair();
    let service = AsyncFilter::new(service, f);
    (service, handle)
}
