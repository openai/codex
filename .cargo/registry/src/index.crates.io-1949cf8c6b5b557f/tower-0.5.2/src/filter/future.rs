//! Future types

use super::AsyncPredicate;
use crate::BoxError;
use futures_core::ready;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower_service::Service;

pin_project! {
    /// Filtered response future from [`AsyncFilter`] services.
    ///
    /// [`AsyncFilter`]: crate::filter::AsyncFilter
    #[derive(Debug)]
    pub struct AsyncResponseFuture<P, S, Request>
    where
        P: AsyncPredicate<Request>,
        S: Service<P::Request>,
    {
        #[pin]
        state: State<P::Future, S::Future>,

        // Inner service
        service: S,
    }
}

opaque_future! {
    /// Filtered response future from [`Filter`] services.
    ///
    /// [`Filter`]: crate::filter::Filter
    pub type ResponseFuture<R, F> =
        futures_util::future::Either<
            futures_util::future::Ready<Result<R, crate::BoxError>>,
            futures_util::future::ErrInto<F, crate::BoxError>
        >;
}

pin_project! {
    #[project = StateProj]
    #[derive(Debug)]
    enum State<F, G> {
        /// Waiting for the predicate future
        Check {
            #[pin]
            check: F
        },
        /// Waiting for the response future
        WaitResponse {
            #[pin]
            response: G
        },
    }
}

impl<P, S, Request> AsyncResponseFuture<P, S, Request>
where
    P: AsyncPredicate<Request>,
    S: Service<P::Request>,
    S::Error: Into<BoxError>,
{
    pub(crate) fn new(check: P::Future, service: S) -> Self {
        Self {
            state: State::Check { check },
            service,
        }
    }
}

impl<P, S, Request> Future for AsyncResponseFuture<P, S, Request>
where
    P: AsyncPredicate<Request>,
    S: Service<P::Request>,
    S::Error: Into<crate::BoxError>,
{
    type Output = Result<S::Response, crate::BoxError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            match this.state.as_mut().project() {
                StateProj::Check { mut check } => {
                    let request = ready!(check.as_mut().poll(cx))?;
                    let response = this.service.call(request);
                    this.state.set(State::WaitResponse { response });
                }
                StateProj::WaitResponse { response } => {
                    return response.poll(cx).map_err(Into::into);
                }
            }
        }
    }
}
