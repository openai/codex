use futures_util::{future::Map, FutureExt};
use std::fmt;
use std::task::{Context, Poll};
use tower_layer::Layer;
use tower_service::Service;

/// Service returned by the [`map_result`] combinator.
///
/// [`map_result`]: crate::util::ServiceExt::map_result
#[derive(Clone)]
pub struct MapResult<S, F> {
    inner: S,
    f: F,
}

impl<S, F> fmt::Debug for MapResult<S, F>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapResult")
            .field("inner", &self.inner)
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .finish()
    }
}

/// A [`Layer`] that produces a [`MapResult`] service.
///
/// [`Layer`]: tower_layer::Layer
#[derive(Debug, Clone)]
pub struct MapResultLayer<F> {
    f: F,
}

opaque_future! {
    /// Response future from [`MapResult`] services.
    ///
    /// [`MapResult`]: crate::util::MapResult
    pub type MapResultFuture<F, N> = Map<F, N>;
}

impl<S, F> MapResult<S, F> {
    /// Creates a new [`MapResult`] service.
    pub const fn new(inner: S, f: F) -> Self {
        MapResult { f, inner }
    }

    /// Returns a new [`Layer`] that produces [`MapResult`] services.
    ///
    /// This is a convenience function that simply calls [`MapResultLayer::new`].
    ///
    /// [`Layer`]: tower_layer::Layer
    pub fn layer(f: F) -> MapResultLayer<F> {
        MapResultLayer { f }
    }
}

impl<S, F, Request, Response, Error> Service<Request> for MapResult<S, F>
where
    S: Service<Request>,
    Error: From<S::Error>,
    F: FnOnce(Result<S::Response, S::Error>) -> Result<Response, Error> + Clone,
{
    type Response = Response;
    type Error = Error;
    type Future = MapResultFuture<S::Future, F>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    #[inline]
    fn call(&mut self, request: Request) -> Self::Future {
        MapResultFuture::new(self.inner.call(request).map(self.f.clone()))
    }
}

impl<F> MapResultLayer<F> {
    /// Creates a new [`MapResultLayer`] layer.
    pub const fn new(f: F) -> Self {
        MapResultLayer { f }
    }
}

impl<S, F> Layer<S> for MapResultLayer<F>
where
    F: Clone,
{
    type Service = MapResult<S, F>;

    fn layer(&self, inner: S) -> Self::Service {
        MapResult {
            f: self.f.clone(),
            inner,
        }
    }
}
