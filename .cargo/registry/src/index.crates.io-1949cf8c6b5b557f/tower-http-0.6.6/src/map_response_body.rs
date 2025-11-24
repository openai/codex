//! Apply a transformation to the response body.
//!
//! # Example
//!
//! ```
//! use bytes::Bytes;
//! use http::{Request, Response};
//! use http_body_util::Full;
//! use std::convert::Infallible;
//! use std::{pin::Pin, task::{ready, Context, Poll}};
//! use tower::{ServiceBuilder, service_fn, ServiceExt, Service};
//! use tower_http::map_response_body::MapResponseBodyLayer;
//!
//! // A wrapper for a `Full<Bytes>`
//! struct BodyWrapper {
//!     inner: Full<Bytes>,
//! }
//!
//! impl BodyWrapper {
//!     fn new(inner: Full<Bytes>) -> Self {
//!         Self { inner }
//!     }
//! }
//!
//! impl http_body::Body for BodyWrapper {
//!     // ...
//!     # type Data = Bytes;
//!     # type Error = tower::BoxError;
//!     # fn poll_frame(
//!     #     self: Pin<&mut Self>,
//!     #     cx: &mut Context<'_>
//!     # ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> { unimplemented!() }
//!     # fn is_end_stream(&self) -> bool { unimplemented!() }
//!     # fn size_hint(&self) -> http_body::SizeHint { unimplemented!() }
//! }
//!
//! async fn handle<B>(_: Request<B>) -> Result<Response<Full<Bytes>>, Infallible> {
//!     // ...
//!     # Ok(Response::new(Full::default()))
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut svc = ServiceBuilder::new()
//!     // Wrap response bodies in `BodyWrapper`
//!     .layer(MapResponseBodyLayer::new(BodyWrapper::new))
//!     .service_fn(handle);
//!
//! // Call the service
//! let request = Request::new(Full::<Bytes>::from("foobar"));
//!
//! svc.ready().await?.call(request).await?;
//! # Ok(())
//! # }
//! ```

use http::{Request, Response};
use pin_project_lite::pin_project;
use std::future::Future;
use std::{
    fmt,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// Apply a transformation to the response body.
///
/// See the [module docs](crate::map_response_body) for an example.
#[derive(Clone)]
pub struct MapResponseBodyLayer<F> {
    f: F,
}

impl<F> MapResponseBodyLayer<F> {
    /// Create a new [`MapResponseBodyLayer`].
    ///
    /// `F` is expected to be a function that takes a body and returns another body.
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

impl<S, F> Layer<S> for MapResponseBodyLayer<F>
where
    F: Clone,
{
    type Service = MapResponseBody<S, F>;

    fn layer(&self, inner: S) -> Self::Service {
        MapResponseBody::new(inner, self.f.clone())
    }
}

impl<F> fmt::Debug for MapResponseBodyLayer<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapResponseBodyLayer")
            .field("f", &std::any::type_name::<F>())
            .finish()
    }
}

/// Apply a transformation to the response body.
///
/// See the [module docs](crate::map_response_body) for an example.
#[derive(Clone)]
pub struct MapResponseBody<S, F> {
    inner: S,
    f: F,
}

impl<S, F> MapResponseBody<S, F> {
    /// Create a new [`MapResponseBody`].
    ///
    /// `F` is expected to be a function that takes a body and returns another body.
    pub fn new(service: S, f: F) -> Self {
        Self { inner: service, f }
    }

    /// Returns a new [`Layer`] that wraps services with a `MapResponseBodyLayer` middleware.
    ///
    /// [`Layer`]: tower_layer::Layer
    pub fn layer(f: F) -> MapResponseBodyLayer<F> {
        MapResponseBodyLayer::new(f)
    }

    define_inner_service_accessors!();
}

impl<F, S, ReqBody, ResBody, NewResBody> Service<Request<ReqBody>> for MapResponseBody<S, F>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    F: FnMut(ResBody) -> NewResBody + Clone,
{
    type Response = Response<NewResBody>;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future, F>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        ResponseFuture {
            inner: self.inner.call(req),
            f: self.f.clone(),
        }
    }
}

impl<S, F> fmt::Debug for MapResponseBody<S, F>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapResponseBody")
            .field("inner", &self.inner)
            .field("f", &std::any::type_name::<F>())
            .finish()
    }
}

pin_project! {
    /// Response future for [`MapResponseBody`].
    pub struct ResponseFuture<Fut, F> {
        #[pin]
        inner: Fut,
        f: F,
    }
}

impl<Fut, F, ResBody, E, NewResBody> Future for ResponseFuture<Fut, F>
where
    Fut: Future<Output = Result<Response<ResBody>, E>>,
    F: FnMut(ResBody) -> NewResBody,
{
    type Output = Result<Response<NewResBody>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let res = ready!(this.inner.poll(cx)?);
        Poll::Ready(Ok(res.map(this.f)))
    }
}
