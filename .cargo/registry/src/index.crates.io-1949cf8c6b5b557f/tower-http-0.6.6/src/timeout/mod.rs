//! Middleware that applies a timeout to requests.
//!
//! If the request does not complete within the specified timeout it will be aborted and a `408
//! Request Timeout` response will be sent.
//!
//! # Differences from `tower::timeout`
//!
//! tower's [`Timeout`](tower::timeout::Timeout) middleware uses an error to signal timeout, i.e.
//! it changes the error type to [`BoxError`](tower::BoxError). For HTTP services that is rarely
//! what you want as returning errors will terminate the connection without sending a response.
//!
//! This middleware won't change the error type and instead return a `408 Request Timeout`
//! response. That means if your service's error type is [`Infallible`] it will still be
//! [`Infallible`] after applying this middleware.
//!
//! # Example
//!
//! ```
//! use http::{Request, Response};
//! use http_body_util::Full;
//! use bytes::Bytes;
//! use std::{convert::Infallible, time::Duration};
//! use tower::ServiceBuilder;
//! use tower_http::timeout::TimeoutLayer;
//!
//! async fn handle(_: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, Infallible> {
//!     // ...
//!     # Ok(Response::new(Full::default()))
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let svc = ServiceBuilder::new()
//!     // Timeout requests after 30 seconds
//!     .layer(TimeoutLayer::new(Duration::from_secs(30)))
//!     .service_fn(handle);
//! # Ok(())
//! # }
//! ```
//!
//! [`Infallible`]: std::convert::Infallible

mod body;
mod service;

pub use body::{TimeoutBody, TimeoutError};
pub use service::{
    RequestBodyTimeout, RequestBodyTimeoutLayer, ResponseBodyTimeout, ResponseBodyTimeoutLayer,
    Timeout, TimeoutLayer,
};
