//! Middleware to override status codes.
//!
//! # Example
//!
//! ```
//! use tower_http::set_status::SetStatusLayer;
//! use http::{Request, Response, StatusCode};
//! use bytes::Bytes;
//! use http_body_util::Full;
//! use std::{iter::once, convert::Infallible};
//! use tower::{ServiceBuilder, Service, ServiceExt};
//!
//! async fn handle(req: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, Infallible> {
//!     // ...
//!     # Ok(Response::new(Full::default()))
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut service = ServiceBuilder::new()
//!     // change the status to `404 Not Found` regardless what the inner service returns
//!     .layer(SetStatusLayer::new(StatusCode::NOT_FOUND))
//!     .service_fn(handle);
//!
//! // Call the service.
//! let request = Request::builder().body(Full::default())?;
//!
//! let response = service.ready().await?.call(request).await?;
//!
//! assert_eq!(response.status(), StatusCode::NOT_FOUND);
//! #
//! # Ok(())
//! # }
//! ```

use http::{Request, Response, StatusCode};
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// Layer that applies [`SetStatus`] which overrides the status codes.
#[derive(Debug, Clone, Copy)]
pub struct SetStatusLayer {
    status: StatusCode,
}

impl SetStatusLayer {
    /// Create a new [`SetStatusLayer`].
    ///
    /// The response status code will be `status` regardless of what the inner service returns.
    pub fn new(status: StatusCode) -> Self {
        SetStatusLayer { status }
    }
}

impl<S> Layer<S> for SetStatusLayer {
    type Service = SetStatus<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SetStatus::new(inner, self.status)
    }
}

/// Middleware to override status codes.
///
/// See the [module docs](self) for more details.
#[derive(Debug, Clone, Copy)]
pub struct SetStatus<S> {
    inner: S,
    status: StatusCode,
}

impl<S> SetStatus<S> {
    /// Create a new [`SetStatus`].
    ///
    /// The response status code will be `status` regardless of what the inner service returns.
    pub fn new(inner: S, status: StatusCode) -> Self {
        Self { status, inner }
    }

    define_inner_service_accessors!();

    /// Returns a new [`Layer`] that wraps services with a `SetStatus` middleware.
    ///
    /// [`Layer`]: tower_layer::Layer
    pub fn layer(status: StatusCode) -> SetStatusLayer {
        SetStatusLayer::new(status)
    }
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for SetStatus<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        ResponseFuture {
            inner: self.inner.call(req),
            status: Some(self.status),
        }
    }
}

pin_project! {
    /// Response future for [`SetStatus`].
    pub struct ResponseFuture<F> {
        #[pin]
        inner: F,
        status: Option<StatusCode>,
    }
}

impl<F, B, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response<B>, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let mut response = ready!(this.inner.poll(cx)?);
        *response.status_mut() = this.status.take().expect("future polled after completion");
        Poll::Ready(Ok(response))
    }
}
