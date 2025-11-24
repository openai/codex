use futures_util::{future, FutureExt};
use std::{
    fmt,
    future::Future,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// [`Service`] returned by the [`then`] combinator.
///
/// [`then`]: crate::util::ServiceExt::then
#[derive(Clone)]
pub struct Then<S, F> {
    inner: S,
    f: F,
}

impl<S, F> fmt::Debug for Then<S, F>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Then")
            .field("inner", &self.inner)
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .finish()
    }
}

/// A [`Layer`] that produces a [`Then`] service.
///
/// [`Layer`]: tower_layer::Layer
#[derive(Debug, Clone)]
pub struct ThenLayer<F> {
    f: F,
}

impl<S, F> Then<S, F> {
    /// Creates a new `Then` service.
    pub const fn new(inner: S, f: F) -> Self {
        Then { f, inner }
    }

    /// Returns a new [`Layer`] that produces [`Then`] services.
    ///
    /// This is a convenience function that simply calls [`ThenLayer::new`].
    ///
    /// [`Layer`]: tower_layer::Layer
    pub fn layer(f: F) -> ThenLayer<F> {
        ThenLayer { f }
    }
}

opaque_future! {
    /// Response future from [`Then`] services.
    ///
    /// [`Then`]: crate::util::Then
    pub type ThenFuture<F1, F2, N> = future::Then<F1, F2, N>;
}

impl<S, F, Request, Response, Error, Fut> Service<Request> for Then<S, F>
where
    S: Service<Request>,
    S::Error: Into<Error>,
    F: FnOnce(Result<S::Response, S::Error>) -> Fut + Clone,
    Fut: Future<Output = Result<Response, Error>>,
{
    type Response = Response;
    type Error = Error;
    type Future = ThenFuture<S::Future, Fut, F>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    #[inline]
    fn call(&mut self, request: Request) -> Self::Future {
        ThenFuture::new(self.inner.call(request).then(self.f.clone()))
    }
}

impl<F> ThenLayer<F> {
    /// Creates a new [`ThenLayer`] layer.
    pub const fn new(f: F) -> Self {
        ThenLayer { f }
    }
}

impl<S, F> Layer<S> for ThenLayer<F>
where
    F: Clone,
{
    type Service = Then<S, F>;

    fn layer(&self, inner: S) -> Self::Service {
        Then {
            f: self.f.clone(),
            inner,
        }
    }
}
