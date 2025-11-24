//! A constant [`Load`] implementation.

#[cfg(feature = "discover")]
use crate::discover::{Change, Discover};
#[cfg(feature = "discover")]
use futures_core::{ready, Stream};
#[cfg(feature = "discover")]
use std::pin::Pin;

use super::Load;
use pin_project_lite::pin_project;
use std::task::{Context, Poll};
use tower_service::Service;

pin_project! {
    #[derive(Debug)]
    /// Wraps a type so that it implements [`Load`] and returns a constant load metric.
    ///
    /// This load estimator is primarily useful for testing.
    pub struct Constant<T, M> {
        inner: T,
        load: M,
    }
}

// ===== impl Constant =====

impl<T, M: Copy> Constant<T, M> {
    /// Wraps a `T`-typed service with a constant `M`-typed load metric.
    pub const fn new(inner: T, load: M) -> Self {
        Self { inner, load }
    }
}

impl<T, M: Copy + PartialOrd> Load for Constant<T, M> {
    type Metric = M;

    fn load(&self) -> M {
        self.load
    }
}

impl<S, M, Request> Service<Request> for Constant<S, M>
where
    S: Service<Request>,
    M: Copy,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        self.inner.call(req)
    }
}

/// Proxies [`Discover`] such that all changes are wrapped with a constant load.
#[cfg(feature = "discover")]
impl<D: Discover + Unpin, M: Copy> Stream for Constant<D, M> {
    type Item = Result<Change<D::Key, Constant<D::Service, M>>, D::Error>;

    /// Yields the next discovery change set.
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use self::Change::*;

        let this = self.project();
        let change = match ready!(Pin::new(this.inner).poll_discover(cx)).transpose()? {
            None => return Poll::Ready(None),
            Some(Insert(k, svc)) => Insert(k, Constant::new(svc, *this.load)),
            Some(Remove(k)) => Remove(k),
        };

        Poll::Ready(Some(Ok(change)))
    }
}
