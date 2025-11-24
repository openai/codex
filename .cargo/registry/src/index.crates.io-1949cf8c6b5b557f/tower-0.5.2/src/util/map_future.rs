use std::{
    fmt,
    future::Future,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// [`Service`] returned by the [`map_future`] combinator.
///
/// [`map_future`]: crate::util::ServiceExt::map_future
#[derive(Clone)]
pub struct MapFuture<S, F> {
    inner: S,
    f: F,
}

impl<S, F> MapFuture<S, F> {
    /// Creates a new [`MapFuture`] service.
    pub const fn new(inner: S, f: F) -> Self {
        Self { inner, f }
    }

    /// Returns a new [`Layer`] that produces [`MapFuture`] services.
    ///
    /// This is a convenience function that simply calls [`MapFutureLayer::new`].
    ///
    /// [`Layer`]: tower_layer::Layer
    pub fn layer(f: F) -> MapFutureLayer<F> {
        MapFutureLayer::new(f)
    }

    /// Get a reference to the inner service
    pub fn get_ref(&self) -> &S {
        &self.inner
    }

    /// Get a mutable reference to the inner service
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.inner
    }

    /// Consume `self`, returning the inner service
    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<R, S, F, T, E, Fut> Service<R> for MapFuture<S, F>
where
    S: Service<R>,
    F: FnMut(S::Future) -> Fut,
    E: From<S::Error>,
    Fut: Future<Output = Result<T, E>>,
{
    type Response = T;
    type Error = E;
    type Future = Fut;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(From::from)
    }

    fn call(&mut self, req: R) -> Self::Future {
        (self.f)(self.inner.call(req))
    }
}

impl<S, F> fmt::Debug for MapFuture<S, F>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapFuture")
            .field("inner", &self.inner)
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .finish()
    }
}

/// A [`Layer`] that produces a [`MapFuture`] service.
///
/// [`Layer`]: tower_layer::Layer
#[derive(Clone)]
pub struct MapFutureLayer<F> {
    f: F,
}

impl<F> MapFutureLayer<F> {
    /// Creates a new [`MapFutureLayer`] layer.
    pub const fn new(f: F) -> Self {
        Self { f }
    }
}

impl<S, F> Layer<S> for MapFutureLayer<F>
where
    F: Clone,
{
    type Service = MapFuture<S, F>;

    fn layer(&self, inner: S) -> Self::Service {
        MapFuture::new(inner, self.f.clone())
    }
}

impl<F> fmt::Debug for MapFutureLayer<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapFutureLayer")
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .finish()
    }
}
