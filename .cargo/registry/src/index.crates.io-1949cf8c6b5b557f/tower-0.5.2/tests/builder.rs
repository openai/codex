#![cfg(all(feature = "buffer", feature = "limit", feature = "retry"))]
mod support;
use futures_util::{future::Ready, pin_mut};
use std::time::Duration;
use tower::builder::ServiceBuilder;
use tower::retry::Policy;
use tower::util::ServiceExt;
use tower_service::*;
use tower_test::{assert_request_eq, mock};

#[tokio::test(flavor = "current_thread")]
async fn builder_service() {
    let _t = support::trace_init();

    let (service, handle) = mock::pair();
    pin_mut!(handle);

    let policy = MockPolicy::<&'static str, bool>::default();
    let mut client = ServiceBuilder::new()
        .buffer(5)
        .concurrency_limit(5)
        .rate_limit(5, Duration::from_secs(5))
        .retry(policy)
        .map_response(|r: &'static str| r == "world")
        .map_request(|r: &'static str| r == "hello")
        .service(service);

    // allow a request through
    handle.allow(1);

    let fut = client.ready().await.unwrap().call("hello");
    assert_request_eq!(handle, true).send_response("world");
    assert!(fut.await.unwrap());
}

#[derive(Debug, Clone, Default)]
struct MockPolicy<Req, Res> {
    _pd: std::marker::PhantomData<(Req, Res)>,
}

impl<Req, Res, E> Policy<Req, Res, E> for MockPolicy<Req, Res>
where
    Req: Clone,
    E: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
{
    type Future = Ready<()>;

    fn retry(&mut self, _req: &mut Req, _result: &mut Result<Res, E>) -> Option<Self::Future> {
        None
    }

    fn clone_request(&mut self, req: &Req) -> Option<Req> {
        Some(req.clone())
    }
}
