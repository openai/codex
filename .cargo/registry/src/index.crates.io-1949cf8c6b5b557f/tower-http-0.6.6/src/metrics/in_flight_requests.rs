//! Measure the number of in-flight requests.
//!
//! In-flight requests is the number of requests a service is currently processing. The processing
//! of a request starts when it is received by the service (`tower::Service::call` is called) and
//! is considered complete when the response body is consumed, dropped, or an error happens.
//!
//! # Example
//!
//! ```
//! use tower::{Service, ServiceExt, ServiceBuilder};
//! use tower_http::metrics::InFlightRequestsLayer;
//! use http::{Request, Response};
//! use bytes::Bytes;
//! use http_body_util::Full;
//! use std::{time::Duration, convert::Infallible};
//!
//! async fn handle(req: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, Infallible> {
//!     // ...
//!     # Ok(Response::new(Full::default()))
//! }
//!
//! async fn update_in_flight_requests_metric(count: usize) {
//!     // ...
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a `Layer` with an associated counter.
//! let (in_flight_requests_layer, counter) = InFlightRequestsLayer::pair();
//!
//! // Spawn a task that will receive the number of in-flight requests every 10 seconds.
//! tokio::spawn(
//!     counter.run_emitter(Duration::from_secs(10), |count| async move {
//!         update_in_flight_requests_metric(count).await;
//!     }),
//! );
//!
//! let mut service = ServiceBuilder::new()
//!     // Keep track of the number of in-flight requests. This will increment and decrement
//!     // `counter` automatically.
//!     .layer(in_flight_requests_layer)
//!     .service_fn(handle);
//!
//! // Call the service.
//! let response = service
//!     .ready()
//!     .await?
//!     .call(Request::new(Full::default()))
//!     .await?;
//! # Ok(())
//! # }
//! ```

use http::{Request, Response};
use http_body::Body;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{ready, Context, Poll},
    time::Duration,
};
use tower_layer::Layer;
use tower_service::Service;

/// Layer for applying [`InFlightRequests`] which counts the number of in-flight requests.
///
/// See the [module docs](crate::metrics::in_flight_requests) for more details.
#[derive(Clone, Debug)]
pub struct InFlightRequestsLayer {
    counter: InFlightRequestsCounter,
}

impl InFlightRequestsLayer {
    /// Create a new `InFlightRequestsLayer` and its associated counter.
    pub fn pair() -> (Self, InFlightRequestsCounter) {
        let counter = InFlightRequestsCounter::new();
        let layer = Self::new(counter.clone());
        (layer, counter)
    }

    /// Create a new `InFlightRequestsLayer` that will update the given counter.
    pub fn new(counter: InFlightRequestsCounter) -> Self {
        Self { counter }
    }
}

impl<S> Layer<S> for InFlightRequestsLayer {
    type Service = InFlightRequests<S>;

    fn layer(&self, inner: S) -> Self::Service {
        InFlightRequests {
            inner,
            counter: self.counter.clone(),
        }
    }
}

/// Middleware that counts the number of in-flight requests.
///
/// See the [module docs](crate::metrics::in_flight_requests) for more details.
#[derive(Clone, Debug)]
pub struct InFlightRequests<S> {
    inner: S,
    counter: InFlightRequestsCounter,
}

impl<S> InFlightRequests<S> {
    /// Create a new `InFlightRequests` and its associated counter.
    pub fn pair(inner: S) -> (Self, InFlightRequestsCounter) {
        let counter = InFlightRequestsCounter::new();
        let service = Self::new(inner, counter.clone());
        (service, counter)
    }

    /// Create a new `InFlightRequests` that will update the given counter.
    pub fn new(inner: S, counter: InFlightRequestsCounter) -> Self {
        Self { inner, counter }
    }

    define_inner_service_accessors!();
}

/// An atomic counter that keeps track of the number of in-flight requests.
///
/// This will normally combined with [`InFlightRequestsLayer`] or [`InFlightRequests`] which will
/// update the counter as requests arrive.
#[derive(Debug, Clone, Default)]
pub struct InFlightRequestsCounter {
    count: Arc<AtomicUsize>,
}

impl InFlightRequestsCounter {
    /// Create a new `InFlightRequestsCounter`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the current number of in-flight requests.
    pub fn get(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }

    fn increment(&self) -> IncrementGuard {
        self.count.fetch_add(1, Ordering::Relaxed);
        IncrementGuard {
            count: self.count.clone(),
        }
    }

    /// Run a future every `interval` which receives the current number of in-flight requests.
    ///
    /// This can be used to send the current count to your metrics system.
    ///
    /// This function will loop forever so normally it is called with [`tokio::spawn`]:
    ///
    /// ```rust,no_run
    /// use tower_http::metrics::in_flight_requests::InFlightRequestsCounter;
    /// use std::time::Duration;
    ///
    /// let counter = InFlightRequestsCounter::new();
    ///
    /// tokio::spawn(
    ///     counter.run_emitter(Duration::from_secs(10), |count: usize| async move {
    ///         // Send `count` to metrics system.
    ///     }),
    /// );
    /// ```
    pub async fn run_emitter<F, Fut>(mut self, interval: Duration, mut emit: F)
    where
        F: FnMut(usize) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send,
    {
        let mut interval = tokio::time::interval(interval);

        loop {
            // if all producers have gone away we don't need to emit anymore
            match Arc::try_unwrap(self.count) {
                Ok(_) => return,
                Err(shared_count) => {
                    self = Self {
                        count: shared_count,
                    }
                }
            }

            interval.tick().await;
            emit(self.get()).await;
        }
    }
}

struct IncrementGuard {
    count: Arc<AtomicUsize>,
}

impl Drop for IncrementGuard {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::Relaxed);
    }
}

impl<S, R, ResBody> Service<Request<R>> for InFlightRequests<S>
where
    S: Service<Request<R>, Response = Response<ResBody>>,
{
    type Response = Response<ResponseBody<ResBody>>;
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<R>) -> Self::Future {
        let guard = self.counter.increment();
        ResponseFuture {
            inner: self.inner.call(req),
            guard: Some(guard),
        }
    }
}

pin_project! {
    /// Response future for [`InFlightRequests`].
    pub struct ResponseFuture<F> {
        #[pin]
        inner: F,
        guard: Option<IncrementGuard>,
    }
}

impl<F, B, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response<B>, E>>,
{
    type Output = Result<Response<ResponseBody<B>>, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let response = ready!(this.inner.poll(cx))?;
        let guard = this.guard.take().unwrap();
        let response = response.map(move |body| ResponseBody { inner: body, guard });

        Poll::Ready(Ok(response))
    }
}

pin_project! {
    /// Response body for [`InFlightRequests`].
    pub struct ResponseBody<B> {
        #[pin]
        inner: B,
        guard: IncrementGuard,
    }
}

impl<B> Body for ResponseBody<B>
where
    B: Body,
{
    type Data = B::Data;
    type Error = B::Error;

    #[inline]
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        self.project().inner.poll_frame(cx)
    }

    #[inline]
    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    #[inline]
    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use crate::test_helpers::Body;
    use http::Request;
    use tower::{BoxError, ServiceBuilder};

    #[tokio::test]
    async fn basic() {
        let (in_flight_requests_layer, counter) = InFlightRequestsLayer::pair();

        let mut service = ServiceBuilder::new()
            .layer(in_flight_requests_layer)
            .service_fn(echo);
        assert_eq!(counter.get(), 0);

        // driving service to ready shouldn't increment the counter
        std::future::poll_fn(|cx| service.poll_ready(cx))
            .await
            .unwrap();
        assert_eq!(counter.get(), 0);

        // creating the response future should increment the count
        let response_future = service.call(Request::new(Body::empty()));
        assert_eq!(counter.get(), 1);

        // count shouldn't decrement until the full body has been comsumed
        let response = response_future.await.unwrap();
        assert_eq!(counter.get(), 1);

        let body = response.into_body();
        crate::test_helpers::to_bytes(body).await.unwrap();
        assert_eq!(counter.get(), 0);
    }

    async fn echo(req: Request<Body>) -> Result<Response<Body>, BoxError> {
        Ok(Response::new(req.into_body()))
    }
}
