//! Convert panics into responses.
//!
//! Note that using panics for error handling is _not_ recommended. Prefer instead to use `Result`
//! whenever possible.
//!
//! # Example
//!
//! ```rust
//! use http::{Request, Response, header::HeaderName};
//! use std::convert::Infallible;
//! use tower::{Service, ServiceExt, ServiceBuilder, service_fn};
//! use tower_http::catch_panic::CatchPanicLayer;
//! use http_body_util::Full;
//! use bytes::Bytes;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! async fn handle(req: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, Infallible> {
//!     panic!("something went wrong...")
//! }
//!
//! let mut svc = ServiceBuilder::new()
//!     // Catch panics and convert them into responses.
//!     .layer(CatchPanicLayer::new())
//!     .service_fn(handle);
//!
//! // Call the service.
//! let request = Request::new(Full::default());
//!
//! let response = svc.ready().await?.call(request).await?;
//!
//! assert_eq!(response.status(), 500);
//! #
//! # Ok(())
//! # }
//! ```
//!
//! Using a custom panic handler:
//!
//! ```rust
//! use http::{Request, StatusCode, Response, header::{self, HeaderName}};
//! use std::{any::Any, convert::Infallible};
//! use tower::{Service, ServiceExt, ServiceBuilder, service_fn};
//! use tower_http::catch_panic::CatchPanicLayer;
//! use bytes::Bytes;
//! use http_body_util::Full;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! async fn handle(req: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, Infallible> {
//!     panic!("something went wrong...")
//! }
//!
//! fn handle_panic(err: Box<dyn Any + Send + 'static>) -> Response<Full<Bytes>> {
//!     let details = if let Some(s) = err.downcast_ref::<String>() {
//!         s.clone()
//!     } else if let Some(s) = err.downcast_ref::<&str>() {
//!         s.to_string()
//!     } else {
//!         "Unknown panic message".to_string()
//!     };
//!
//!     let body = serde_json::json!({
//!         "error": {
//!             "kind": "panic",
//!             "details": details,
//!         }
//!     });
//!     let body = serde_json::to_string(&body).unwrap();
//!
//!     Response::builder()
//!         .status(StatusCode::INTERNAL_SERVER_ERROR)
//!         .header(header::CONTENT_TYPE, "application/json")
//!         .body(Full::from(body))
//!         .unwrap()
//! }
//!
//! let svc = ServiceBuilder::new()
//!     // Use `handle_panic` to create the response.
//!     .layer(CatchPanicLayer::custom(handle_panic))
//!     .service_fn(handle);
//! #
//! # Ok(())
//! # }
//! ```

use bytes::Bytes;
use futures_util::future::{CatchUnwind, FutureExt};
use http::{HeaderValue, Request, Response, StatusCode};
use http_body::Body;
use http_body_util::BodyExt;
use pin_project_lite::pin_project;
use std::{
    any::Any,
    future::Future,
    panic::AssertUnwindSafe,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

use crate::{
    body::{Full, UnsyncBoxBody},
    BoxError,
};

/// Layer that applies the [`CatchPanic`] middleware that catches panics and converts them into
/// `500 Internal Server` responses.
///
/// See the [module docs](self) for an example.
#[derive(Debug, Clone, Copy, Default)]
pub struct CatchPanicLayer<T> {
    panic_handler: T,
}

impl CatchPanicLayer<DefaultResponseForPanic> {
    /// Create a new `CatchPanicLayer` with the default panic handler.
    pub fn new() -> Self {
        CatchPanicLayer {
            panic_handler: DefaultResponseForPanic,
        }
    }
}

impl<T> CatchPanicLayer<T> {
    /// Create a new `CatchPanicLayer` with a custom panic handler.
    pub fn custom(panic_handler: T) -> Self
    where
        T: ResponseForPanic,
    {
        Self { panic_handler }
    }
}

impl<T, S> Layer<S> for CatchPanicLayer<T>
where
    T: Clone,
{
    type Service = CatchPanic<S, T>;

    fn layer(&self, inner: S) -> Self::Service {
        CatchPanic {
            inner,
            panic_handler: self.panic_handler.clone(),
        }
    }
}

/// Middleware that catches panics and converts them into `500 Internal Server` responses.
///
/// See the [module docs](self) for an example.
#[derive(Debug, Clone, Copy)]
pub struct CatchPanic<S, T> {
    inner: S,
    panic_handler: T,
}

impl<S> CatchPanic<S, DefaultResponseForPanic> {
    /// Create a new `CatchPanic` with the default panic handler.
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            panic_handler: DefaultResponseForPanic,
        }
    }
}

impl<S, T> CatchPanic<S, T> {
    define_inner_service_accessors!();

    /// Create a new `CatchPanic` with a custom panic handler.
    pub fn custom(inner: S, panic_handler: T) -> Self
    where
        T: ResponseForPanic,
    {
        Self {
            inner,
            panic_handler,
        }
    }
}

impl<S, T, ReqBody, ResBody> Service<Request<ReqBody>> for CatchPanic<S, T>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    ResBody: Body<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T: ResponseForPanic + Clone,
    T::ResponseBody: Body<Data = Bytes> + Send + 'static,
    <T::ResponseBody as Body>::Error: Into<BoxError>,
{
    type Response = Response<UnsyncBoxBody<Bytes, BoxError>>;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future, T>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        match std::panic::catch_unwind(AssertUnwindSafe(|| self.inner.call(req))) {
            Ok(future) => ResponseFuture {
                kind: Kind::Future {
                    future: AssertUnwindSafe(future).catch_unwind(),
                    panic_handler: Some(self.panic_handler.clone()),
                },
            },
            Err(panic_err) => ResponseFuture {
                kind: Kind::Panicked {
                    panic_err: Some(panic_err),
                    panic_handler: Some(self.panic_handler.clone()),
                },
            },
        }
    }
}

pin_project! {
    /// Response future for [`CatchPanic`].
    pub struct ResponseFuture<F, T> {
        #[pin]
        kind: Kind<F, T>,
    }
}

pin_project! {
    #[project = KindProj]
    enum Kind<F, T> {
        Panicked {
            panic_err: Option<Box<dyn Any + Send + 'static>>,
            panic_handler: Option<T>,
        },
        Future {
            #[pin]
            future: CatchUnwind<AssertUnwindSafe<F>>,
            panic_handler: Option<T>,
        }
    }
}

impl<F, ResBody, E, T> Future for ResponseFuture<F, T>
where
    F: Future<Output = Result<Response<ResBody>, E>>,
    ResBody: Body<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T: ResponseForPanic,
    T::ResponseBody: Body<Data = Bytes> + Send + 'static,
    <T::ResponseBody as Body>::Error: Into<BoxError>,
{
    type Output = Result<Response<UnsyncBoxBody<Bytes, BoxError>>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().kind.project() {
            KindProj::Panicked {
                panic_err,
                panic_handler,
            } => {
                let panic_handler = panic_handler
                    .take()
                    .expect("future polled after completion");
                let panic_err = panic_err.take().expect("future polled after completion");
                Poll::Ready(Ok(response_for_panic(panic_handler, panic_err)))
            }
            KindProj::Future {
                future,
                panic_handler,
            } => match ready!(future.poll(cx)) {
                Ok(Ok(res)) => {
                    Poll::Ready(Ok(res.map(|body| {
                        UnsyncBoxBody::new(body.map_err(Into::into).boxed_unsync())
                    })))
                }
                Ok(Err(svc_err)) => Poll::Ready(Err(svc_err)),
                Err(panic_err) => Poll::Ready(Ok(response_for_panic(
                    panic_handler
                        .take()
                        .expect("future polled after completion"),
                    panic_err,
                ))),
            },
        }
    }
}

fn response_for_panic<T>(
    mut panic_handler: T,
    err: Box<dyn Any + Send + 'static>,
) -> Response<UnsyncBoxBody<Bytes, BoxError>>
where
    T: ResponseForPanic,
    T::ResponseBody: Body<Data = Bytes> + Send + 'static,
    <T::ResponseBody as Body>::Error: Into<BoxError>,
{
    panic_handler
        .response_for_panic(err)
        .map(|body| UnsyncBoxBody::new(body.map_err(Into::into).boxed_unsync()))
}

/// Trait for creating responses from panics.
pub trait ResponseForPanic: Clone {
    /// The body type used for responses to panics.
    type ResponseBody;

    /// Create a response from the panic error.
    fn response_for_panic(
        &mut self,
        err: Box<dyn Any + Send + 'static>,
    ) -> Response<Self::ResponseBody>;
}

impl<F, B> ResponseForPanic for F
where
    F: FnMut(Box<dyn Any + Send + 'static>) -> Response<B> + Clone,
{
    type ResponseBody = B;

    fn response_for_panic(
        &mut self,
        err: Box<dyn Any + Send + 'static>,
    ) -> Response<Self::ResponseBody> {
        self(err)
    }
}

/// The default `ResponseForPanic` used by `CatchPanic`.
///
/// It will log the panic message and return a `500 Internal Server` error response with an empty
/// body.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct DefaultResponseForPanic;

impl ResponseForPanic for DefaultResponseForPanic {
    type ResponseBody = Full;

    fn response_for_panic(
        &mut self,
        err: Box<dyn Any + Send + 'static>,
    ) -> Response<Self::ResponseBody> {
        if let Some(s) = err.downcast_ref::<String>() {
            tracing::error!("Service panicked: {}", s);
        } else if let Some(s) = err.downcast_ref::<&str>() {
            tracing::error!("Service panicked: {}", s);
        } else {
            tracing::error!(
                "Service panicked but `CatchPanic` was unable to downcast the panic info"
            );
        };

        let mut res = Response::new(Full::new(http_body_util::Full::from("Service panicked")));
        *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;

        #[allow(clippy::declare_interior_mutable_const)]
        const TEXT_PLAIN: HeaderValue = HeaderValue::from_static("text/plain; charset=utf-8");
        res.headers_mut()
            .insert(http::header::CONTENT_TYPE, TEXT_PLAIN);

        res
    }
}

#[cfg(test)]
mod tests {
    #![allow(unreachable_code)]

    use super::*;
    use crate::test_helpers::Body;
    use http::Response;
    use std::convert::Infallible;
    use tower::{ServiceBuilder, ServiceExt};

    #[tokio::test]
    async fn panic_before_returning_future() {
        let svc = ServiceBuilder::new()
            .layer(CatchPanicLayer::new())
            .service_fn(|_: Request<Body>| {
                panic!("service panic");
                async { Ok::<_, Infallible>(Response::new(Body::empty())) }
            });

        let req = Request::new(Body::empty());

        let res = svc.oneshot(req).await.unwrap();

        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = crate::test_helpers::to_bytes(res).await.unwrap();
        assert_eq!(&body[..], b"Service panicked");
    }

    #[tokio::test]
    async fn panic_in_future() {
        let svc = ServiceBuilder::new()
            .layer(CatchPanicLayer::new())
            .service_fn(|_: Request<Body>| async {
                panic!("future panic");
                Ok::<_, Infallible>(Response::new(Body::empty()))
            });

        let req = Request::new(Body::empty());

        let res = svc.oneshot(req).await.unwrap();

        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = crate::test_helpers::to_bytes(res).await.unwrap();
        assert_eq!(&body[..], b"Service panicked");
    }
}
