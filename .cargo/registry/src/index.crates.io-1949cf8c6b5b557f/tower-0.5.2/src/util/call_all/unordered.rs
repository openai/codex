//! [`Stream<Item = Request>`][stream] + [`Service<Request>`] => [`Stream<Item = Response>`][stream].
//!
//! [`Service<Request>`]: crate::Service
//! [stream]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html

use super::common;
use futures_core::Stream;
use futures_util::stream::FuturesUnordered;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower_service::Service;

pin_project! {
    /// A stream of responses received from the inner service in received order.
    ///
    /// Similar to [`CallAll`] except, instead of yielding responses in request order,
    /// responses are returned as they are available.
    ///
    /// [`CallAll`]: crate::util::CallAll
    #[derive(Debug)]
    pub struct CallAllUnordered<Svc, S>
    where
        Svc: Service<S::Item>,
        S: Stream,
    {
        #[pin]
        inner: common::CallAll<Svc, S, FuturesUnordered<Svc::Future>>,
    }
}

impl<Svc, S> CallAllUnordered<Svc, S>
where
    Svc: Service<S::Item>,
    S: Stream,
{
    /// Create new [`CallAllUnordered`] combinator.
    ///
    /// [`Stream`]: https://docs.rs/futures/latest/futures/stream/trait.Stream.html
    pub fn new(service: Svc, stream: S) -> CallAllUnordered<Svc, S> {
        CallAllUnordered {
            inner: common::CallAll::new(service, stream, FuturesUnordered::new()),
        }
    }

    /// Extract the wrapped [`Service`].
    ///
    /// # Panics
    ///
    /// Panics if [`take_service`] was already called.
    ///
    /// [`take_service`]: crate::util::CallAllUnordered::take_service
    pub fn into_inner(self) -> Svc {
        self.inner.into_inner()
    }

    /// Extract the wrapped `Service`.
    ///
    /// This [`CallAllUnordered`] can no longer be used after this function has been called.
    ///
    /// # Panics
    ///
    /// Panics if [`take_service`] was already called.
    ///
    /// [`take_service`]: crate::util::CallAllUnordered::take_service
    pub fn take_service(self: Pin<&mut Self>) -> Svc {
        self.project().inner.take_service()
    }
}

impl<Svc, S> Stream for CallAllUnordered<Svc, S>
where
    Svc: Service<S::Item>,
    S: Stream,
{
    type Item = Result<Svc::Response, Svc::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx)
    }
}

impl<F: Future> common::Drive<F> for FuturesUnordered<F> {
    fn is_empty(&self) -> bool {
        FuturesUnordered::is_empty(self)
    }

    fn push(&mut self, future: F) {
        FuturesUnordered::push(self, future)
    }

    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Option<F::Output>> {
        Stream::poll_next(Pin::new(self), cx)
    }
}
