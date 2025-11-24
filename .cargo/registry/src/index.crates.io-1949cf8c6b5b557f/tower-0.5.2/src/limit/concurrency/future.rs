//! [`Future`] types
//!
//! [`Future`]: std::future::Future
use futures_core::ready;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::OwnedSemaphorePermit;

pin_project! {
    /// Future for the [`ConcurrencyLimit`] service.
    ///
    /// [`ConcurrencyLimit`]: crate::limit::ConcurrencyLimit
    #[derive(Debug)]
    pub struct ResponseFuture<T> {
        #[pin]
        inner: T,
        // Keep this around so that it is dropped when the future completes
        _permit: OwnedSemaphorePermit,
    }
}

impl<T> ResponseFuture<T> {
    pub(crate) fn new(inner: T, _permit: OwnedSemaphorePermit) -> ResponseFuture<T> {
        ResponseFuture { inner, _permit }
    }
}

impl<F, T, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<T, E>>,
{
    type Output = Result<T, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(ready!(self.project().inner.poll(cx)))
    }
}
