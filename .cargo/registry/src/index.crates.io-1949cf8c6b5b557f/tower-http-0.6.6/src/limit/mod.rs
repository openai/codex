//! Middleware for limiting request bodies.
//!
//! This layer will also intercept requests with a `Content-Length` header
//! larger than the allowable limit and return an immediate error response
//! before reading any of the body.
//!
//! Note that payload length errors can be used by adversaries in an attempt
//! to smuggle requests. When an incoming stream is dropped due to an
//! over-sized payload, servers should close the connection or resynchronize
//! by optimistically consuming some data in an attempt to reach the end of
//! the current HTTP frame. If the incoming stream cannot be resynchronized,
//! then the connection should be closed. If you're using [hyper] this is
//! automatically handled for you.
//!
//! # Examples
//!
//! ## Limiting based on `Content-Length`
//!
//! If a `Content-Length` header is present and indicates a payload that is
//! larger than the acceptable limit, then the underlying service will not
//! be called and a `413 Payload Too Large` response will be generated.
//!
//! ```rust
//! use bytes::Bytes;
//! use std::convert::Infallible;
//! use http::{Request, Response, StatusCode, HeaderValue, header::CONTENT_LENGTH};
//! use http_body_util::{LengthLimitError};
//! use tower::{Service, ServiceExt, ServiceBuilder};
//! use tower_http::{body::Limited, limit::RequestBodyLimitLayer};
//! use http_body_util::Full;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! async fn handle(req: Request<Limited<Full<Bytes>>>) -> Result<Response<Full<Bytes>>, Infallible> {
//!     panic!("This should not be hit")
//! }
//!
//! let mut svc = ServiceBuilder::new()
//!     // Limit incoming requests to 4096 bytes.
//!     .layer(RequestBodyLimitLayer::new(4096))
//!     .service_fn(handle);
//!
//! // Call the service with a header that indicates the body is too large.
//! let mut request = Request::builder()
//!     .header(CONTENT_LENGTH, HeaderValue::from_static("5000"))
//!     .body(Full::<Bytes>::default())
//!     .unwrap();
//!
//! // let response = svc.ready().await?.call(request).await?;
//! let response = svc.call(request).await?;
//!
//! assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
//! #
//! # Ok(())
//! # }
//! ```
//!
//! ## Limiting without known `Content-Length`
//!
//! If a `Content-Length` header is not present, then the body will be read
//! until the configured limit has been reached. If the payload is larger than
//! the limit, the [`http_body_util::Limited`] body will return an error. This
//! error can be inspected to determine if it is a [`http_body_util::LengthLimitError`]
//! and return an appropriate response in such case.
//!
//! Note that no error will be generated if the body is never read. Similarly,
//! if the body _would be_ to large, but is never consumed beyond the length
//! limit, then no error is generated, and handling of the remaining incoming
//! data stream is left to the server implementation as described above.
//!
//! ```rust
//! # use bytes::Bytes;
//! # use std::convert::Infallible;
//! # use http::{Request, Response, StatusCode};
//! # use http_body_util::LengthLimitError;
//! # use tower::{Service, ServiceExt, ServiceBuilder, BoxError};
//! # use tower_http::{body::Limited, limit::RequestBodyLimitLayer};
//! # use http_body_util::Full;
//! # use http_body_util::BodyExt;
//! #
//! # #[tokio::main]
//! # async fn main() -> Result<(), BoxError> {
//! async fn handle(req: Request<Limited<Full<Bytes>>>) -> Result<Response<Full<Bytes>>, BoxError> {
//!     let data = match req.into_body().collect().await {
//!         Ok(collected) => collected.to_bytes(),
//!         Err(err) => {
//!             if let Some(_) = err.downcast_ref::<LengthLimitError>() {
//!                 let mut resp = Response::new(Full::default());
//!                 *resp.status_mut() = StatusCode::PAYLOAD_TOO_LARGE;
//!                 return Ok(resp);
//!             } else {
//!                 return Err(err);
//!             }
//!         }
//!     };
//!
//!     Ok(Response::new(Full::default()))
//! }
//!
//! let mut svc = ServiceBuilder::new()
//!     // Limit incoming requests to 4096 bytes.
//!     .layer(RequestBodyLimitLayer::new(4096))
//!     .service_fn(handle);
//!
//! // Call the service.
//! let request = Request::new(Full::<Bytes>::default());
//!
//! let response = svc.ready().await?.call(request).await?;
//!
//! assert_eq!(response.status(), StatusCode::OK);
//!
//! // Call the service with a body that is too large.
//! let request = Request::new(Full::<Bytes>::from(Bytes::from(vec![0u8; 4097])));
//!
//! let response = svc.ready().await?.call(request).await?;
//!
//! assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
//! #
//! # Ok(())
//! # }
//! ```
//!
//! ## Limiting without `Content-Length`
//!
//! If enforcement of body size limits is desired without preemptively
//! handling requests with a `Content-Length` header indicating an over-sized
//! request, consider using [`MapRequestBody`] to wrap the request body with
//! [`http_body_util::Limited`] and checking for [`http_body_util::LengthLimitError`]
//! like in the previous example.
//!
//! [`MapRequestBody`]: crate::map_request_body
//! [hyper]: https://crates.io/crates/hyper

mod body;
mod future;
mod layer;
mod service;

pub use body::ResponseBody;
pub use future::ResponseFuture;
pub use layer::RequestBodyLimitLayer;
pub use service::RequestBodyLimit;
