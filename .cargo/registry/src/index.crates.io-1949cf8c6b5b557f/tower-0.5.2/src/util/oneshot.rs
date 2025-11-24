use futures_core::ready;
use pin_project_lite::pin_project;
use std::{
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower_service::Service;

pin_project! {
    /// A [`Future`] consuming a [`Service`] and request, waiting until the [`Service`]
    /// is ready, and then calling [`Service::call`] with the request, and
    /// waiting for that [`Future`].
    #[derive(Debug)]
    pub struct Oneshot<S: Service<Req>, Req> {
        #[pin]
        state: State<S, Req>,
    }
}

pin_project! {
    #[project = StateProj]
    enum State<S: Service<Req>, Req> {
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

impl<S: Service<Req>, Req> State<S, Req> {
    const fn not_ready(svc: S, req: Option<Req>) -> Self {
        Self::NotReady { svc, req }
    }

    const fn called(fut: S::Future) -> Self {
        Self::Called { fut }
    }
}

impl<S, Req> fmt::Debug for State<S, Req>
where
    S: Service<Req> + fmt::Debug,
    Req: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            State::NotReady {
                svc,
                req: Some(req),
            } => f
                .debug_tuple("State::NotReady")
                .field(svc)
                .field(req)
                .finish(),
            State::NotReady { req: None, .. } => unreachable!(),
            State::Called { .. } => f.debug_tuple("State::Called").field(&"S::Future").finish(),
            State::Done => f.debug_tuple("State::Done").finish(),
        }
    }
}

impl<S, Req> Oneshot<S, Req>
where
    S: Service<Req>,
{
    #[allow(missing_docs)]
    pub const fn new(svc: S, req: Req) -> Self {
        Oneshot {
            state: State::not_ready(svc, Some(req)),
        }
    }
}

impl<S, Req> Future for Oneshot<S, Req>
where
    S: Service<Req>,
{
    type Output = Result<S::Response, S::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        loop {
            match this.state.as_mut().project() {
                StateProj::NotReady { svc, req } => {
                    let _ = ready!(svc.poll_ready(cx))?;
                    let f = svc.call(req.take().expect("already called"));
                    this.state.set(State::called(f));
                }
                StateProj::Called { fut } => {
                    let res = ready!(fut.poll(cx))?;
                    this.state.set(State::Done);
                    return Poll::Ready(Ok(res));
                }
                StateProj::Done => panic!("polled after complete"),
            }
        }
    }
}
