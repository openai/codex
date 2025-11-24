use futures_core::TryFuture;
use futures_util::{future, TryFutureExt};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower_layer::Layer;
use tower_service::Service;

/// Service returned by the [`and_then`] combinator.
///
/// [`and_then`]: crate::util::ServiceExt::and_then
#[derive(Clone)]
pub struct AndThen<S, F> {
    inner: S,
    f: F,
}

impl<S, F> fmt::Debug for AndThen<S, F>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AndThen")
            .field("inner", &self.inner)
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .finish()
    }
}

pin_project_lite::pin_project! {
    /// Response future from [`AndThen`] services.
    ///
    /// [`AndThen`]: crate::util::AndThen
    pub struct AndThenFuture<F1, F2: TryFuture, N> {
        #[pin]
        inner: future::AndThen<future::ErrInto<F1, F2::Error>, F2, N>,
    }
}

impl<F1, F2: TryFuture, N> AndThenFuture<F1, F2, N> {
    pub(crate) fn new(inner: future::AndThen<future::ErrInto<F1, F2::Error>, F2, N>) -> Self {
        Self { inner }
    }
}

impl<F1, F2: TryFuture, N> std::fmt::Debug for AndThenFuture<F1, F2, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AndThenFuture")
            .field(&format_args!("..."))
            .finish()
    }
}

impl<F1, F2: TryFuture, N> Future for AndThenFuture<F1, F2, N>
where
    future::AndThen<future::ErrInto<F1, F2::Error>, F2, N>: Future,
{
    type Output = <future::AndThen<future::ErrInto<F1, F2::Error>, F2, N> as Future>::Output;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}

/// A [`Layer`] that produces a [`AndThen`] service.
///
/// [`Layer`]: tower_layer::Layer
#[derive(Clone, Debug)]
pub struct AndThenLayer<F> {
    f: F,
}

impl<S, F> AndThen<S, F> {
    /// Creates a new `AndThen` service.
    pub const fn new(inner: S, f: F) -> Self {
        AndThen { f, inner }
    }

    /// Returns a new [`Layer`] that produces [`AndThen`] services.
    ///
    /// This is a convenience function that simply calls [`AndThenLayer::new`].
    ///
    /// [`Layer`]: tower_layer::Layer
    pub fn layer(f: F) -> AndThenLayer<F> {
        AndThenLayer { f }
    }
}

impl<S, F, Request, Fut> Service<Request> for AndThen<S, F>
where
    S: Service<Request>,
    S::Error: Into<Fut::Error>,
    F: FnOnce(S::Response) -> Fut + Clone,
    Fut: TryFuture,
{
    type Response = Fut::Ok;
    type Error = Fut::Error;
    type Future = AndThenFuture<S::Future, Fut, F>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        AndThenFuture::new(self.inner.call(request).err_into().and_then(self.f.clone()))
    }
}

impl<F> AndThenLayer<F> {
    /// Creates a new [`AndThenLayer`] layer.
    pub const fn new(f: F) -> Self {
        AndThenLayer { f }
    }
}

impl<S, F> Layer<S> for AndThenLayer<F>
where
    F: Clone,
{
    type Service = AndThen<S, F>;

    fn layer(&self, inner: S) -> Self::Service {
        AndThen {
            f: self.f.clone(),
            inner,
        }
    }
}
