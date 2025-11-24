use futures_core::ready;
use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower_service::Service;

// Vendored from tower::util to reduce dependencies, the code is small enough.

// Not really pub, but used in a trait for bounds
pin_project! {
    #[project = OneshotProj]
    #[derive(Debug)]
    pub enum Oneshot<S: Service<Req>, Req> {
        NotReady {
            svc: S,
            req: Option<Req>,
        },
        Called {
            #[pin]
            fut: S::Future,
        },
        Done,
    }
}

impl<S, Req> Oneshot<S, Req>
where
    S: Service<Req>,
{
    pub(crate) const fn new(svc: S, req: Req) -> Self {
        Oneshot::NotReady {
            svc,
            req: Some(req),
        }
    }
}

impl<S, Req> Future for Oneshot<S, Req>
where
    S: Service<Req>,
{
    type Output = Result<S::Response, S::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let this = self.as_mut().project();
            match this {
                OneshotProj::NotReady { svc, req } => {
                    ready!(svc.poll_ready(cx))?;
                    let fut = svc.call(req.take().expect("already called"));
                    self.set(Oneshot::Called { fut });
                }
                OneshotProj::Called { fut } => {
                    let res = ready!(fut.poll(cx))?;
                    self.set(Oneshot::Done);
                    return Poll::Ready(Ok(res));
                }
                OneshotProj::Done => panic!("polled after complete"),
            }
        }
    }
}
