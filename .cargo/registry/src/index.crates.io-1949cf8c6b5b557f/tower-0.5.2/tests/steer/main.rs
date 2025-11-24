#![cfg(feature = "steer")]
#[path = "../support.rs"]
mod support;

use futures_util::future::{ready, Ready};
use std::task::{Context, Poll};
use tower::steer::Steer;
use tower_service::Service;

type StdError = Box<dyn std::error::Error + Send + Sync + 'static>;

struct MyService(u8, bool);

impl Service<String> for MyService {
    type Response = u8;
    type Error = StdError;
    type Future = Ready<Result<u8, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if !self.1 {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn call(&mut self, _req: String) -> Self::Future {
        ready(Ok(self.0))
    }
}

#[tokio::test(flavor = "current_thread")]
async fn pick_correctly() {
    let _t = support::trace_init();
    let srvs = vec![MyService(42, true), MyService(57, true)];
    let mut st = Steer::new(srvs, |_: &_, _: &[_]| 1);

    futures_util::future::poll_fn(|cx| st.poll_ready(cx))
        .await
        .unwrap();
    let r = st.call(String::from("foo")).await.unwrap();
    assert_eq!(r, 57);
}

#[tokio::test(flavor = "current_thread")]
async fn pending_all_ready() {
    let _t = support::trace_init();

    let srvs = vec![MyService(42, true), MyService(57, false)];
    let mut st = Steer::new(srvs, |_: &_, _: &[_]| 0);

    let p = futures_util::poll!(futures_util::future::poll_fn(|cx| st.poll_ready(cx)));
    match p {
        Poll::Pending => (),
        _ => panic!(
            "Steer should not return poll_ready if at least one component service is not ready"
        ),
    }
}
