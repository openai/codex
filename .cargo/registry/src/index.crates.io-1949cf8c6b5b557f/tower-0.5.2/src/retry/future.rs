//! Future types

use super::{Policy, Retry};
use futures_core::ready;
use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower_service::Service;

pin_project! {
    /// The [`Future`] returned by a [`Retry`] service.
    #[derive(Debug)]
    pub struct ResponseFuture<P, S, Request>
    where
        P: Policy<Request, S::Response, S::Error>,
        S: Service<Request>,
    {
        request: Option<Request>,
        #[pin]
        retry: Retry<P, S>,
        #[pin]
        state: State<S::Future, P::Future>,
    }
}

pin_project! {
    #[project = StateProj]
    #[derive(Debug)]
    enum State<F, P> {
        // Polling the future from [`Service::call`]
        Called {
            #[pin]
            future: F
        },
        // Polling the future from [`Policy::retry`]
        Waiting {
            #[pin]
            waiting: P
        },
        // Polling [`Service::poll_ready`] after [`Waiting`] was OK.
        Retrying,
    }
}

impl<P, S, Request> ResponseFuture<P, S, Request>
where
    P: Policy<Request, S::Response, S::Error>,
    S: Service<Request>,
{
    pub(crate) fn new(
        request: Option<Request>,
        retry: Retry<P, S>,
        future: S::Future,
    ) -> ResponseFuture<P, S, Request> {
        ResponseFuture {
            request,
            retry,
            state: State::Called { future },
        }
    }
}

impl<P, S, Request> Future for ResponseFuture<P, S, Request>
where
    P: Policy<Request, S::Response, S::Error>,
    S: Service<Request>,
{
    type Output = Result<S::Response, S::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            match this.state.as_mut().project() {
                StateProj::Called { future } => {
                    let mut result = ready!(future.poll(cx));
                    if let Some(req) = &mut this.request {
                        match this.retry.policy.retry(req, &mut result) {
                            Some(waiting) => {
                                this.state.set(State::Waiting { waiting });
                            }
                            None => return Poll::Ready(result),
                        }
                    } else {
                        // request wasn't cloned, so no way to retry it
                        return Poll::Ready(result);
                    }
                }
                StateProj::Waiting { waiting } => {
                    ready!(waiting.poll(cx));

                    this.state.set(State::Retrying);
                }
                StateProj::Retrying => {
                    // NOTE: we assume here that
                    //
                    //   this.retry.poll_ready()
                    //
                    // is equivalent to
                    //
                    //   this.retry.service.poll_ready()
                    //
                    // we need to make that assumption to avoid adding an Unpin bound to the Policy
                    // in Ready to make it Unpin so that we can get &mut Ready as needed to call
                    // poll_ready on it.
                    ready!(this.retry.as_mut().project().service.poll_ready(cx))?;
                    let req = this
                        .request
                        .take()
                        .expect("retrying requires cloned request");
                    *this.request = this.retry.policy.clone_request(&req);
                    this.state.set(State::Called {
                        future: this.retry.as_mut().project().service.call(req),
                    });
                }
            }
        }
    }
}
