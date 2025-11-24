//! Middleware which recovers from error.

use std::{
    fmt,
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};

use http::Response;
use pin_project::pin_project;
use tower_layer::Layer;
use tower_service::Service;

use crate::Status;

/// Layer which applies the [`RecoverError`] middleware.
#[derive(Debug, Default, Clone)]
pub struct RecoverErrorLayer {
    _priv: (),
}

impl RecoverErrorLayer {
    /// Create a new `RecoverErrorLayer`.
    pub fn new() -> Self {
        Self { _priv: () }
    }
}

impl<S> Layer<S> for RecoverErrorLayer {
    type Service = RecoverError<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RecoverError::new(inner)
    }
}

/// Middleware that attempts to recover from service errors by turning them into a response built
/// from the `Status`.
#[derive(Debug, Clone)]
pub struct RecoverError<S> {
    inner: S,
}

impl<S> RecoverError<S> {
    /// Create a new `RecoverError` middleware.
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, Req, ResBody> Service<Req> for RecoverError<S>
where
    S: Service<Req, Response = Response<ResBody>>,
    S::Error: Into<crate::BoxError>,
{
    type Response = Response<ResponseBody<ResBody>>;
    type Error = crate::BoxError;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: Req) -> Self::Future {
        ResponseFuture {
            inner: self.inner.call(req),
        }
    }
}

/// Response future for [`RecoverError`].
#[pin_project]
pub struct ResponseFuture<F> {
    #[pin]
    inner: F,
}

impl<F> fmt::Debug for ResponseFuture<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseFuture").finish()
    }
}

impl<F, E, ResBody> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response<ResBody>, E>>,
    E: Into<crate::BoxError>,
{
    type Output = Result<Response<ResponseBody<ResBody>>, crate::BoxError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match ready!(self.project().inner.poll(cx)) {
            Ok(response) => {
                let response = response.map(ResponseBody::full);
                Poll::Ready(Ok(response))
            }
            Err(err) => match Status::try_from_error(err.into()) {
                Ok(status) => {
                    let (parts, ()) = status.into_http::<()>().into_parts();
                    let res = Response::from_parts(parts, ResponseBody::empty());
                    Poll::Ready(Ok(res))
                }
                Err(err) => Poll::Ready(Err(err)),
            },
        }
    }
}

/// Response body for [`RecoverError`].
#[pin_project]
pub struct ResponseBody<B> {
    #[pin]
    inner: Option<B>,
}

impl<B> fmt::Debug for ResponseBody<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseBody").finish()
    }
}

impl<B> ResponseBody<B> {
    fn full(inner: B) -> Self {
        Self { inner: Some(inner) }
    }

    const fn empty() -> Self {
        Self { inner: None }
    }
}

impl<B> http_body::Body for ResponseBody<B>
where
    B: http_body::Body,
{
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        match self.project().inner.as_pin_mut() {
            Some(b) => b.poll_frame(cx),
            None => Poll::Ready(None),
        }
    }

    fn is_end_stream(&self) -> bool {
        match &self.inner {
            Some(b) => b.is_end_stream(),
            None => true,
        }
    }

    fn size_hint(&self) -> http_body::SizeHint {
        match &self.inner {
            Some(body) => body.size_hint(),
            None => http_body::SizeHint::with_exact(0),
        }
    }
}
