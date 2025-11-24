//! Middleware that adds high level [tracing] to a [`Service`].
//!
//! # Example
//!
//! Adding tracing to your service can be as simple as:
//!
//! ```rust
//! use http::{Request, Response};
//! use tower::{ServiceBuilder, ServiceExt, Service};
//! use tower_http::trace::TraceLayer;
//! use std::convert::Infallible;
//! use http_body_util::Full;
//! use bytes::Bytes;
//!
//! async fn handle(request: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, Infallible> {
//!     Ok(Response::new(Full::default()))
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Setup tracing
//! tracing_subscriber::fmt::init();
//!
//! let mut service = ServiceBuilder::new()
//!     .layer(TraceLayer::new_for_http())
//!     .service_fn(handle);
//!
//! let request = Request::new(Full::from("foo"));
//!
//! let response = service
//!     .ready()
//!     .await?
//!     .call(request)
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! If you run this application with `RUST_LOG=tower_http=trace cargo run` you should see logs like:
//!
//! ```text
//! Mar 05 20:50:28.523 DEBUG request{method=GET path="/foo"}: tower_http::trace::on_request: started processing request
//! Mar 05 20:50:28.524 DEBUG request{method=GET path="/foo"}: tower_http::trace::on_response: finished processing request latency=1 ms status=200
//! ```
//!
//! # Customization
//!
//! [`Trace`] comes with good defaults but also supports customizing many aspects of the output.
//!
//! The default behaviour supports some customization:
//!
//! ```rust
//! use http::{Request, Response, HeaderMap, StatusCode};
//! use http_body_util::Full;
//! use bytes::Bytes;
//! use tower::ServiceBuilder;
//! use tracing::Level;
//! use tower_http::{
//!     LatencyUnit,
//!     trace::{TraceLayer, DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse},
//! };
//! use std::time::Duration;
//! # use tower::{ServiceExt, Service};
//! # use std::convert::Infallible;
//!
//! # async fn handle(request: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, Infallible> {
//! #     Ok(Response::new(Full::from("foo")))
//! # }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # tracing_subscriber::fmt::init();
//! #
//! let service = ServiceBuilder::new()
//!     .layer(
//!         TraceLayer::new_for_http()
//!             .make_span_with(
//!                 DefaultMakeSpan::new().include_headers(true)
//!             )
//!             .on_request(
//!                 DefaultOnRequest::new().level(Level::INFO)
//!             )
//!             .on_response(
//!                 DefaultOnResponse::new()
//!                     .level(Level::INFO)
//!                     .latency_unit(LatencyUnit::Micros)
//!             )
//!             // on so on for `on_eos`, `on_body_chunk`, and `on_failure`
//!     )
//!     .service_fn(handle);
//! # let mut service = service;
//! # let response = service
//! #     .ready()
//! #     .await?
//! #     .call(Request::new(Full::from("foo")))
//! #     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! However for maximum control you can provide callbacks:
//!
//! ```rust
//! use http::{Request, Response, HeaderMap, StatusCode};
//! use http_body_util::Full;
//! use bytes::Bytes;
//! use tower::ServiceBuilder;
//! use tower_http::{classify::ServerErrorsFailureClass, trace::TraceLayer};
//! use std::time::Duration;
//! use tracing::Span;
//! # use tower::{ServiceExt, Service};
//! # use std::convert::Infallible;
//!
//! # async fn handle(request: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, Infallible> {
//! #     Ok(Response::new(Full::from("foo")))
//! # }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # tracing_subscriber::fmt::init();
//! #
//! let service = ServiceBuilder::new()
//!     .layer(
//!         TraceLayer::new_for_http()
//!             .make_span_with(|request: &Request<Full<Bytes>>| {
//!                 tracing::debug_span!("http-request")
//!             })
//!             .on_request(|request: &Request<Full<Bytes>>, _span: &Span| {
//!                 tracing::debug!("started {} {}", request.method(), request.uri().path())
//!             })
//!             .on_response(|response: &Response<Full<Bytes>>, latency: Duration, _span: &Span| {
//!                 tracing::debug!("response generated in {:?}", latency)
//!             })
//!             .on_body_chunk(|chunk: &Bytes, latency: Duration, _span: &Span| {
//!                 tracing::debug!("sending {} bytes", chunk.len())
//!             })
//!             .on_eos(|trailers: Option<&HeaderMap>, stream_duration: Duration, _span: &Span| {
//!                 tracing::debug!("stream closed after {:?}", stream_duration)
//!             })
//!             .on_failure(|error: ServerErrorsFailureClass, latency: Duration, _span: &Span| {
//!                 tracing::debug!("something went wrong")
//!             })
//!     )
//!     .service_fn(handle);
//! # let mut service = service;
//! # let response = service
//! #     .ready()
//! #     .await?
//! #     .call(Request::new(Full::from("foo")))
//! #     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Disabling something
//!
//! Setting the behaviour to `()` will be disable that particular step:
//!
//! ```rust
//! use http::StatusCode;
//! use tower::ServiceBuilder;
//! use tower_http::{classify::ServerErrorsFailureClass, trace::TraceLayer};
//! use std::time::Duration;
//! use tracing::Span;
//! # use tower::{ServiceExt, Service};
//! # use http_body_util::Full;
//! # use bytes::Bytes;
//! # use http::{Response, Request};
//! # use std::convert::Infallible;
//!
//! # async fn handle(request: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, Infallible> {
//! #     Ok(Response::new(Full::from("foo")))
//! # }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # tracing_subscriber::fmt::init();
//! #
//! let service = ServiceBuilder::new()
//!     .layer(
//!         // This configuration will only emit events on failures
//!         TraceLayer::new_for_http()
//!             .on_request(())
//!             .on_response(())
//!             .on_body_chunk(())
//!             .on_eos(())
//!             .on_failure(|error: ServerErrorsFailureClass, latency: Duration, _span: &Span| {
//!                 tracing::debug!("something went wrong")
//!             })
//!     )
//!     .service_fn(handle);
//! # let mut service = service;
//! # let response = service
//! #     .ready()
//! #     .await?
//! #     .call(Request::new(Full::from("foo")))
//! #     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! # When the callbacks are called
//!
//! ### `on_request`
//!
//! The `on_request` callback is called when the request arrives at the
//! middleware in [`Service::call`] just prior to passing the request to the
//! inner service.
//!
//! ### `on_response`
//!
//! The `on_response` callback is called when the inner service's response
//! future completes with `Ok(response)` regardless if the response is
//! classified as a success or a failure.
//!
//! For example if you're using [`ServerErrorsAsFailures`] as your classifier
//! and the inner service responds with `500 Internal Server Error` then the
//! `on_response` callback is still called. `on_failure` would _also_ be called
//! in this case since the response was classified as a failure.
//!
//! ### `on_body_chunk`
//!
//! The `on_body_chunk` callback is called when the response body produces a new
//! chunk, that is when [`Body::poll_frame`] returns a data frame.
//!
//! `on_body_chunk` is called even if the chunk is empty.
//!
//! ### `on_eos`
//!
//! The `on_eos` callback is called when a streaming response body ends, that is
//! when [`Body::poll_frame`] returns a trailers frame.
//!
//! `on_eos` is called even if the trailers produced are `None`.
//!
//! ### `on_failure`
//!
//! The `on_failure` callback is called when:
//!
//! - The inner [`Service`]'s response future resolves to an error.
//! - A response is classified as a failure.
//! - [`Body::poll_frame`] returns an error.
//! - An end-of-stream is classified as a failure.
//!
//! # Recording fields on the span
//!
//! All callbacks receive a reference to the [tracing] [`Span`], corresponding to this request,
//! produced by the closure passed to [`TraceLayer::make_span_with`]. It can be used to [record
//! field values][record] that weren't known when the span was created.
//!
//! ```rust
//! use http::{Request, Response, HeaderMap, StatusCode};
//! use http_body_util::Full;
//! use bytes::Bytes;
//! use tower::ServiceBuilder;
//! use tower_http::trace::TraceLayer;
//! use tracing::Span;
//! use std::time::Duration;
//! # use std::convert::Infallible;
//!
//! # async fn handle(request: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, Infallible> {
//! #     Ok(Response::new(Full::from("foo")))
//! # }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # tracing_subscriber::fmt::init();
//! #
//! let service = ServiceBuilder::new()
//!     .layer(
//!         TraceLayer::new_for_http()
//!             .make_span_with(|request: &Request<Full<Bytes>>| {
//!                 tracing::debug_span!(
//!                     "http-request",
//!                     status_code = tracing::field::Empty,
//!                 )
//!             })
//!             .on_response(|response: &Response<Full<Bytes>>, _latency: Duration, span: &Span| {
//!                 span.record("status_code", &tracing::field::display(response.status()));
//!
//!                 tracing::debug!("response generated")
//!             })
//!     )
//!     .service_fn(handle);
//! # Ok(())
//! # }
//! ```
//!
//! # Providing classifiers
//!
//! Tracing requires determining if a response is a success or failure. [`MakeClassifier`] is used
//! to create a classifier for the incoming request. See the docs for [`MakeClassifier`] and
//! [`ClassifyResponse`] for more details on classification.
//!
//! A [`MakeClassifier`] can be provided when creating a [`TraceLayer`]:
//!
//! ```rust
//! use http::{Request, Response};
//! use http_body_util::Full;
//! use bytes::Bytes;
//! use tower::ServiceBuilder;
//! use tower_http::{
//!     trace::TraceLayer,
//!     classify::{
//!         MakeClassifier, ClassifyResponse, ClassifiedResponse, NeverClassifyEos,
//!         SharedClassifier,
//!     },
//! };
//! use std::convert::Infallible;
//!
//! # async fn handle(request: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, Infallible> {
//! #     Ok(Response::new(Full::from("foo")))
//! # }
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # tracing_subscriber::fmt::init();
//! #
//! // Our `MakeClassifier` that always crates `MyClassifier` classifiers.
//! #[derive(Copy, Clone)]
//! struct MyMakeClassify;
//!
//! impl MakeClassifier for MyMakeClassify {
//!     type Classifier = MyClassifier;
//!     type FailureClass = &'static str;
//!     type ClassifyEos = NeverClassifyEos<&'static str>;
//!
//!     fn make_classifier<B>(&self, req: &Request<B>) -> Self::Classifier {
//!         MyClassifier
//!     }
//! }
//!
//! // A classifier that classifies failures as `"something went wrong..."`.
//! #[derive(Copy, Clone)]
//! struct MyClassifier;
//!
//! impl ClassifyResponse for MyClassifier {
//!     type FailureClass = &'static str;
//!     type ClassifyEos = NeverClassifyEos<&'static str>;
//!
//!     fn classify_response<B>(
//!         self,
//!         res: &Response<B>
//!     ) -> ClassifiedResponse<Self::FailureClass, Self::ClassifyEos> {
//!         // Classify based on the status code.
//!         if res.status().is_server_error() {
//!             ClassifiedResponse::Ready(Err("something went wrong..."))
//!         } else {
//!             ClassifiedResponse::Ready(Ok(()))
//!         }
//!     }
//!
//!     fn classify_error<E>(self, error: &E) -> Self::FailureClass
//!     where
//!         E: std::fmt::Display + 'static,
//!     {
//!         "something went wrong..."
//!     }
//! }
//!
//! let service = ServiceBuilder::new()
//!     // Create a trace layer that uses our classifier.
//!     .layer(TraceLayer::new(MyMakeClassify))
//!     .service_fn(handle);
//!
//! // Since `MyClassifier` is `Clone` we can also use `SharedClassifier`
//! // to avoid having to define a separate `MakeClassifier`.
//! let service = ServiceBuilder::new()
//!     .layer(TraceLayer::new(SharedClassifier::new(MyClassifier)))
//!     .service_fn(handle);
//! # Ok(())
//! # }
//! ```
//!
//! [`TraceLayer`] comes with convenience methods for using common classifiers:
//!
//! - [`TraceLayer::new_for_http`] classifies based on the status code. It doesn't consider
//!   streaming responses.
//! - [`TraceLayer::new_for_grpc`] classifies based on the gRPC protocol and supports streaming
//!   responses.
//!
//! [tracing]: https://crates.io/crates/tracing
//! [`Service`]: tower_service::Service
//! [`Service::call`]: tower_service::Service::call
//! [`MakeClassifier`]: crate::classify::MakeClassifier
//! [`ClassifyResponse`]: crate::classify::ClassifyResponse
//! [record]: https://docs.rs/tracing/latest/tracing/span/struct.Span.html#method.record
//! [`TraceLayer::make_span_with`]: crate::trace::TraceLayer::make_span_with
//! [`Span`]: tracing::Span
//! [`ServerErrorsAsFailures`]: crate::classify::ServerErrorsAsFailures
//! [`Body::poll_frame`]: http_body::Body::poll_frame

use std::{fmt, time::Duration};

use tracing::Level;

pub use self::{
    body::ResponseBody,
    future::ResponseFuture,
    layer::TraceLayer,
    make_span::{DefaultMakeSpan, MakeSpan},
    on_body_chunk::{DefaultOnBodyChunk, OnBodyChunk},
    on_eos::{DefaultOnEos, OnEos},
    on_failure::{DefaultOnFailure, OnFailure},
    on_request::{DefaultOnRequest, OnRequest},
    on_response::{DefaultOnResponse, OnResponse},
    service::Trace,
};
use crate::{
    classify::{GrpcErrorsAsFailures, ServerErrorsAsFailures, SharedClassifier},
    LatencyUnit,
};

/// MakeClassifier for HTTP requests.
pub type HttpMakeClassifier = SharedClassifier<ServerErrorsAsFailures>;

/// MakeClassifier for gRPC requests.
pub type GrpcMakeClassifier = SharedClassifier<GrpcErrorsAsFailures>;

macro_rules! event_dynamic_lvl {
    ( $(target: $target:expr,)? $(parent: $parent:expr,)? $lvl:expr, $($tt:tt)* ) => {
        match $lvl {
            tracing::Level::ERROR => {
                tracing::event!(
                    $(target: $target,)?
                    $(parent: $parent,)?
                    tracing::Level::ERROR,
                    $($tt)*
                );
            }
            tracing::Level::WARN => {
                tracing::event!(
                    $(target: $target,)?
                    $(parent: $parent,)?
                    tracing::Level::WARN,
                    $($tt)*
                );
            }
            tracing::Level::INFO => {
                tracing::event!(
                    $(target: $target,)?
                    $(parent: $parent,)?
                    tracing::Level::INFO,
                    $($tt)*
                );
            }
            tracing::Level::DEBUG => {
                tracing::event!(
                    $(target: $target,)?
                    $(parent: $parent,)?
                    tracing::Level::DEBUG,
                    $($tt)*
                );
            }
            tracing::Level::TRACE => {
                tracing::event!(
                    $(target: $target,)?
                    $(parent: $parent,)?
                    tracing::Level::TRACE,
                    $($tt)*
                );
            }
        }
    };
}

mod body;
mod future;
mod layer;
mod make_span;
mod on_body_chunk;
mod on_eos;
mod on_failure;
mod on_request;
mod on_response;
mod service;

const DEFAULT_MESSAGE_LEVEL: Level = Level::DEBUG;
const DEFAULT_ERROR_LEVEL: Level = Level::ERROR;

struct Latency {
    unit: LatencyUnit,
    duration: Duration,
}

impl fmt::Display for Latency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.unit {
            LatencyUnit::Seconds => write!(f, "{} s", self.duration.as_secs_f64()),
            LatencyUnit::Millis => write!(f, "{} ms", self.duration.as_millis()),
            LatencyUnit::Micros => write!(f, "{} μs", self.duration.as_micros()),
            LatencyUnit::Nanos => write!(f, "{} ns", self.duration.as_nanos()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::ServerErrorsFailureClass;
    use crate::test_helpers::Body;
    use bytes::Bytes;
    use http::{HeaderMap, Request, Response};
    use once_cell::sync::Lazy;
    use std::{
        sync::atomic::{AtomicU32, Ordering},
        time::Duration,
    };
    use tower::{BoxError, Service, ServiceBuilder, ServiceExt};
    use tracing::Span;

    #[tokio::test]
    async fn unary_request() {
        static ON_REQUEST_COUNT: Lazy<AtomicU32> = Lazy::new(|| AtomicU32::new(0));
        static ON_RESPONSE_COUNT: Lazy<AtomicU32> = Lazy::new(|| AtomicU32::new(0));
        static ON_BODY_CHUNK_COUNT: Lazy<AtomicU32> = Lazy::new(|| AtomicU32::new(0));
        static ON_EOS: Lazy<AtomicU32> = Lazy::new(|| AtomicU32::new(0));
        static ON_FAILURE: Lazy<AtomicU32> = Lazy::new(|| AtomicU32::new(0));

        let trace_layer = TraceLayer::new_for_http()
            .make_span_with(|_req: &Request<Body>| {
                tracing::info_span!("test-span", foo = tracing::field::Empty)
            })
            .on_request(|_req: &Request<Body>, span: &Span| {
                span.record("foo", 42);
                ON_REQUEST_COUNT.fetch_add(1, Ordering::SeqCst);
            })
            .on_response(|_res: &Response<Body>, _latency: Duration, _span: &Span| {
                ON_RESPONSE_COUNT.fetch_add(1, Ordering::SeqCst);
            })
            .on_body_chunk(|_chunk: &Bytes, _latency: Duration, _span: &Span| {
                ON_BODY_CHUNK_COUNT.fetch_add(1, Ordering::SeqCst);
            })
            .on_eos(
                |_trailers: Option<&HeaderMap>, _latency: Duration, _span: &Span| {
                    ON_EOS.fetch_add(1, Ordering::SeqCst);
                },
            )
            .on_failure(
                |_class: ServerErrorsFailureClass, _latency: Duration, _span: &Span| {
                    ON_FAILURE.fetch_add(1, Ordering::SeqCst);
                },
            );

        let mut svc = ServiceBuilder::new().layer(trace_layer).service_fn(echo);

        let res = svc
            .ready()
            .await
            .unwrap()
            .call(Request::new(Body::from("foobar")))
            .await
            .unwrap();

        assert_eq!(1, ON_REQUEST_COUNT.load(Ordering::SeqCst), "request");
        assert_eq!(1, ON_RESPONSE_COUNT.load(Ordering::SeqCst), "request");
        assert_eq!(0, ON_BODY_CHUNK_COUNT.load(Ordering::SeqCst), "body chunk");
        assert_eq!(0, ON_EOS.load(Ordering::SeqCst), "eos");
        assert_eq!(0, ON_FAILURE.load(Ordering::SeqCst), "failure");

        crate::test_helpers::to_bytes(res.into_body())
            .await
            .unwrap();
        assert_eq!(1, ON_BODY_CHUNK_COUNT.load(Ordering::SeqCst), "body chunk");
        assert_eq!(0, ON_EOS.load(Ordering::SeqCst), "eos");
        assert_eq!(0, ON_FAILURE.load(Ordering::SeqCst), "failure");
    }

    #[tokio::test]
    async fn streaming_response() {
        static ON_REQUEST_COUNT: Lazy<AtomicU32> = Lazy::new(|| AtomicU32::new(0));
        static ON_RESPONSE_COUNT: Lazy<AtomicU32> = Lazy::new(|| AtomicU32::new(0));
        static ON_BODY_CHUNK_COUNT: Lazy<AtomicU32> = Lazy::new(|| AtomicU32::new(0));
        static ON_EOS: Lazy<AtomicU32> = Lazy::new(|| AtomicU32::new(0));
        static ON_FAILURE: Lazy<AtomicU32> = Lazy::new(|| AtomicU32::new(0));

        let trace_layer = TraceLayer::new_for_http()
            .on_request(|_req: &Request<Body>, _span: &Span| {
                ON_REQUEST_COUNT.fetch_add(1, Ordering::SeqCst);
            })
            .on_response(|_res: &Response<Body>, _latency: Duration, _span: &Span| {
                ON_RESPONSE_COUNT.fetch_add(1, Ordering::SeqCst);
            })
            .on_body_chunk(|_chunk: &Bytes, _latency: Duration, _span: &Span| {
                ON_BODY_CHUNK_COUNT.fetch_add(1, Ordering::SeqCst);
            })
            .on_eos(
                |_trailers: Option<&HeaderMap>, _latency: Duration, _span: &Span| {
                    ON_EOS.fetch_add(1, Ordering::SeqCst);
                },
            )
            .on_failure(
                |_class: ServerErrorsFailureClass, _latency: Duration, _span: &Span| {
                    ON_FAILURE.fetch_add(1, Ordering::SeqCst);
                },
            );

        let mut svc = ServiceBuilder::new()
            .layer(trace_layer)
            .service_fn(streaming_body);

        let res = svc
            .ready()
            .await
            .unwrap()
            .call(Request::new(Body::empty()))
            .await
            .unwrap();

        assert_eq!(1, ON_REQUEST_COUNT.load(Ordering::SeqCst), "request");
        assert_eq!(1, ON_RESPONSE_COUNT.load(Ordering::SeqCst), "request");
        assert_eq!(0, ON_BODY_CHUNK_COUNT.load(Ordering::SeqCst), "body chunk");
        assert_eq!(0, ON_EOS.load(Ordering::SeqCst), "eos");
        assert_eq!(0, ON_FAILURE.load(Ordering::SeqCst), "failure");

        crate::test_helpers::to_bytes(res.into_body())
            .await
            .unwrap();
        assert_eq!(3, ON_BODY_CHUNK_COUNT.load(Ordering::SeqCst), "body chunk");
        assert_eq!(0, ON_EOS.load(Ordering::SeqCst), "eos");
        assert_eq!(0, ON_FAILURE.load(Ordering::SeqCst), "failure");
    }

    async fn echo(req: Request<Body>) -> Result<Response<Body>, BoxError> {
        Ok(Response::new(req.into_body()))
    }

    async fn streaming_body(_req: Request<Body>) -> Result<Response<Body>, BoxError> {
        use futures_util::stream::iter;

        let stream = iter(vec![
            Ok::<_, BoxError>(Bytes::from("one")),
            Ok::<_, BoxError>(Bytes::from("two")),
            Ok::<_, BoxError>(Bytes::from("three")),
        ]);

        let body = Body::from_stream(stream);

        Ok(Response::new(body))
    }
}
