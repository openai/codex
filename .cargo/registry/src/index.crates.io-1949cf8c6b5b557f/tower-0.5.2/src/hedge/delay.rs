use futures_util::ready;
use pin_project_lite::pin_project;
use std::time::Duration;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower_service::Service;

use crate::util::Oneshot;

/// A policy which specifies how long each request should be delayed for.
pub trait Policy<Request> {
    fn delay(&self, req: &Request) -> Duration;
}

/// A middleware which delays sending the request to the underlying service
/// for an amount of time specified by the policy.
#[derive(Debug)]
pub struct Delay<P, S> {
    policy: P,
    service: S,
}

pin_project! {
    #[derive(Debug)]
    pub struct ResponseFuture<Request, S>
    where
        S: Service<Request>,
    {
        service: Option<S>,
        #[pin]
        state: State<Request, Oneshot<S, Request>>,
    }
}

pin_project! {
    #[project = StateProj]
    #[derive(Debug)]
    enum State<Request, F> {
        Delaying {
            #[pin]
            delay: tokio::time::Sleep,
            req: Option<Request>,
        },
        Called {
            #[pin]
            fut: F,
        },
    }
}

impl<Request, F> State<Request, F> {
    fn delaying(delay: tokio::time::Sleep, req: Option<Request>) -> Self {
        Self::Delaying { delay, req }
    }

    fn called(fut: F) -> Self {
        Self::Called { fut }
    }
}

impl<P, S> Delay<P, S> {
    pub const fn new<Request>(policy: P, service: S) -> Self
    where
        P: Policy<Request>,
        S: Service<Request> + Clone,
        S::Error: Into<crate::BoxError>,
    {
        Delay { policy, service }
    }
}

impl<Request, P, S> Service<Request> for Delay<P, S>
where
    P: Policy<Request>,
    S: Service<Request> + Clone,
    S::Error: Into<crate::BoxError>,
{
    type Response = S::Response;
    type Error = crate::BoxError;
    type Future = ResponseFuture<Request, S>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Calling self.service.poll_ready would reserve a slot for the delayed request,
        // potentially well in advance of actually making it.  Instead, signal readiness here and
        // treat the service as a Oneshot in the future.
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let delay = self.policy.delay(&request);
        ResponseFuture {
            service: Some(self.service.clone()),
            state: State::delaying(tokio::time::sleep(delay), Some(request)),
        }
    }
}

impl<Request, S, T, E> Future for ResponseFuture<Request, S>
where
    E: Into<crate::BoxError>,
    S: Service<Request, Response = T, Error = E>,
{
    type Output = Result<T, crate::BoxError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            match this.state.as_mut().project() {
                StateProj::Delaying { delay, req } => {
                    ready!(delay.poll(cx));
                    let req = req.take().expect("Missing request in delay");
                    let svc = this.service.take().expect("Missing service in delay");
                    let fut = Oneshot::new(svc, req);
                    this.state.set(State::called(fut));
                }
                StateProj::Called { fut } => {
                    return fut.poll(cx).map_err(Into::into);
                }
            };
        }
    }
}
