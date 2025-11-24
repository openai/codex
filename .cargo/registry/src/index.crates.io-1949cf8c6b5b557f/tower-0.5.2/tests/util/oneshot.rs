use std::task::{Context, Poll};
use std::{future::Future, pin::Pin};
use tower::util::ServiceExt;
use tower_service::Service;

#[tokio::test(flavor = "current_thread")]
async fn service_driven_to_readiness() {
    // This test ensures that `oneshot` will repeatedly call `poll_ready` until
    // the service is ready.
    let _t = super::support::trace_init();

    struct PollMeTwice {
        ready: bool,
    }
    impl Service<()> for PollMeTwice {
        type Error = ();
        type Response = ();
        type Future = Pin<
            Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + Sync + 'static>,
        >;

        fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), ()>> {
            if self.ready {
                Poll::Ready(Ok(()))
            } else {
                self.ready = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }

        fn call(&mut self, _: ()) -> Self::Future {
            assert!(self.ready, "service not driven to readiness!");
            Box::pin(async { Ok(()) })
        }
    }

    let svc = PollMeTwice { ready: false };
    svc.oneshot(()).await.unwrap();
}
