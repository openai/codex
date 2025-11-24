//! Apply a transformation to the request body.
//!
//! # Example
//!
//! ```
//! use http_body_util::Full;
//! use bytes::Bytes;
//! use http::{Request, Response};
//! use std::convert::Infallible;
//! use std::{pin::Pin, task::{ready, Context, Poll}};
//! use tower::{ServiceBuilder, service_fn, ServiceExt, Service};
//! use tower_http::map_request_body::MapRequestBodyLayer;
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
//!     .layer(MapRequestBodyLayer::new(BodyWrapper::new))
//!     .service_fn(handle);
//!
//! // Call the service
//! let request = Request::new(Full::default());
//!
//! svc.ready().await?.call(request).await?;
//! # Ok(())
//! # }
//! ```

use http::{Request, Response};
use std::{
    fmt,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// Apply a transformation to the request body.
///
/// See the [module docs](crate::map_request_body) for an example.
#[derive(Clone)]
pub struct MapRequestBodyLayer<F> {
    f: F,
}

impl<F> MapRequestBodyLayer<F> {
    /// Create a new [`MapRequestBodyLayer`].
    ///
    /// `F` is expected to be a function that takes a body and returns another body.
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

impl<S, F> Layer<S> for MapRequestBodyLayer<F>
where
    F: Clone,
{
    type Service = MapRequestBody<S, F>;

    fn layer(&self, inner: S) -> Self::Service {
        MapRequestBody::new(inner, self.f.clone())
    }
}

impl<F> fmt::Debug for MapRequestBodyLayer<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapRequestBodyLayer")
            .field("f", &std::any::type_name::<F>())
            .finish()
    }
}

/// Apply a transformation to the request body.
///
/// See the [module docs](crate::map_request_body) for an example.
#[derive(Clone)]
pub struct MapRequestBody<S, F> {
    inner: S,
    f: F,
}

impl<S, F> MapRequestBody<S, F> {
    /// Create a new [`MapRequestBody`].
    ///
    /// `F` is expected to be a function that takes a body and returns another body.
    pub fn new(service: S, f: F) -> Self {
        Self { inner: service, f }
    }

    /// Returns a new [`Layer`] that wraps services with a `MapRequestBodyLayer` middleware.
    ///
    /// [`Layer`]: tower_layer::Layer
    pub fn layer(f: F) -> MapRequestBodyLayer<F> {
        MapRequestBodyLayer::new(f)
    }

    define_inner_service_accessors!();
}

impl<F, S, ReqBody, ResBody, NewReqBody> Service<Request<ReqBody>> for MapRequestBody<S, F>
where
    S: Service<Request<NewReqBody>, Response = Response<ResBody>>,
    F: FnMut(ReqBody) -> NewReqBody,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let req = req.map(&mut self.f);
        self.inner.call(req)
    }
}

impl<S, F> fmt::Debug for MapRequestBody<S, F>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapRequestBody")
            .field("inner", &self.inner)
            .field("f", &std::any::type_name::<F>())
            .finish()
    }
}
