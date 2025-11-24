use futures_util::ready;
use pin_project_lite::pin_project;
use std::time::Duration;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::time::Instant;
use tower_service::Service;

/// Record is the interface for accepting request latency measurements.  When
/// a request completes, record is called with the elapsed duration between
/// when the service was called and when the future completed.
pub trait Record {
    fn record(&mut self, latency: Duration);
}

/// Latency is a middleware that measures request latency and records it to the
/// provided Record instance.
#[derive(Clone, Debug)]
pub struct Latency<R, S> {
    rec: R,
    service: S,
}

pin_project! {
    #[derive(Debug)]
    pub struct ResponseFuture<R, F> {
        start: Instant,
        rec: R,
        #[pin]
        inner: F,
    }
}

impl<S, R> Latency<R, S>
where
    R: Record + Clone,
{
    pub const fn new<Request>(rec: R, service: S) -> Self
    where
        S: Service<Request>,
        S::Error: Into<crate::BoxError>,
    {
        Latency { rec, service }
    }
}

impl<S, R, Request> Service<Request> for Latency<R, S>
where
    S: Service<Request>,
    S::Error: Into<crate::BoxError>,
    R: Record + Clone,
{
    type Response = S::Response;
    type Error = crate::BoxError;
    type Future = ResponseFuture<R, S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        ResponseFuture {
            start: Instant::now(),
            rec: self.rec.clone(),
            inner: self.service.call(request),
        }
    }
}

impl<R, F, T, E> Future for ResponseFuture<R, F>
where
    R: Record,
    F: Future<Output = Result<T, E>>,
    E: Into<crate::BoxError>,
{
    type Output = Result<T, crate::BoxError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let rsp = ready!(this.inner.poll(cx)).map_err(Into::into)?;
        let duration = Instant::now().saturating_duration_since(*this.start);
        this.rec.record(duration);
        Poll::Ready(Ok(rsp))
    }
}
